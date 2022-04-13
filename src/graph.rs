use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{atomic::AtomicUsize, Arc, Mutex};

use dashmap::DashSet;
use smol_str::SmolStr;
use tokio::sync::mpsc::{self, Sender};

use crate::{
  ast::{self, module::ModuleId},
  async_worker::{AsyncWorker, WorkerMessage},
  result::Error,
  utils::resolve_id,
};

#[derive(Debug)]
pub struct Graph {
  resolved_entries: Vec<ModuleId>,
  id_to_module: HashMap<ModuleId, ast::module::Module>,
}

#[derive(Debug)]
pub struct GraphOptions<T: AsRef<str>> {
  pub entry: Vec<T>,
}

impl Graph {
  pub fn new<T>(options: GraphOptions<T>) -> Self
  where
    T: AsRef<str>,
  {
    let resolved_entries = options
      .entry
      .iter()
      .map(|item| resolve_id(&nodejs_path::resolve!(item.as_ref())))
      .collect::<Vec<ModuleId>>();

    Self {
      resolved_entries,
      id_to_module: Default::default(),
    }
  }

  pub async fn generate(&mut self) -> Result<(), Error> {
    let num_of_threads = num_cpus::get_physical();
    let idle_thread_count: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(num_of_threads));

    let (tx, mut rx) = mpsc::channel::<WorkerMessage>(32);

    let modules_to_work: Arc<Mutex<Vec<ModuleId>>> =
      Arc::new(Mutex::new(self.resolved_entries.clone()));

    let worked_modules: Arc<DashSet<ModuleId>> = Arc::new(DashSet::new());

    for _ in 0..num_of_threads {
      let idle_thread_count = idle_thread_count.clone();
      let mut async_worker = AsyncWorker {
        resp_tx: tx.clone(),
        modules_to_work: modules_to_work.clone(),
        worked_modules: worked_modules.clone(),
        resolved_entries: Arc::new(DashSet::from_iter(self.resolved_entries.clone())),
      };

      tokio::spawn(async move {
        loop {
          idle_thread_count.fetch_sub(1, Ordering::SeqCst);
          async_worker.run().await;
          idle_thread_count.fetch_add(1, Ordering::SeqCst);

          loop {
            if !async_worker.modules_to_work.lock().unwrap().is_empty() {
              break;
            } else if idle_thread_count.load(Ordering::SeqCst) == num_of_threads {
              return;
            }
          }
        }
      });
    }

    drop(tx);

    while !modules_to_work.lock().unwrap().is_empty()
      || idle_thread_count.load(Ordering::SeqCst) != num_of_threads
    {
      if let Ok(worker_message) = rx.try_recv() {
        use WorkerMessage::*;
        log::debug!("[AsyncWorker] Received new message {:?}", worker_message);
        match worker_message {
          NewModule(module) => {
            self.id_to_module.insert(module.id.clone(), module);
          }
        }
      }
    }

    Ok(())
  }
}
