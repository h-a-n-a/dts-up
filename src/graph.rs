use std::sync::{Arc, Mutex};

use dashmap::DashSet;
use log::debug;
use petgraph::Graph;
use smol_str::SmolStr;
use tokio::sync::mpsc::{self, Sender};

use crate::{
  async_worker::{AsyncWorker, WorkerMessage},
  result::Error,
  utils::resolve_id,
};

#[derive(Debug)]
pub struct ModuleGraph {
  resolved_entries: Vec<SmolStr>,
}

#[derive(Debug)]
pub struct ModuleGraphOptions<T: AsRef<str>> {
  pub entry: Vec<T>,
}

impl ModuleGraph {
  pub fn new<T>(options: ModuleGraphOptions<T>) -> Self
  where
    T: AsRef<str>,
  {
    let resolved_entries = options
      .entry
      .iter()
      .map(|item| resolve_id(item.as_ref()))
      .collect::<Vec<_>>();

    Self { resolved_entries }
  }

  pub async fn generate(&mut self) -> Result<(), Error> {
    let num_of_threads = num_cpus::get_physical();

    let (tx, mut rx) = mpsc::channel::<WorkerMessage>(32);

    let resolved_entries = self
      .resolved_entries
      .iter()
      .cloned()
      .collect::<Vec<SmolStr>>();

    let modules_to_work: Arc<Mutex<Vec<SmolStr>>> = Arc::new(Mutex::new(resolved_entries.clone()));

    let worked_modules: Arc<DashSet<SmolStr>> = Arc::new(DashSet::new());

    for _ in 0..num_of_threads {
      let tx = tx.clone();
      let mut async_worker = AsyncWorker {
        resp_tx: tx,
        modules_to_work: modules_to_work.clone(),
        worked_modules: worked_modules.clone(),
        resolved_entries: Arc::new(DashSet::from_iter(resolved_entries.clone())),
      };

      let a = tokio::spawn(async move {
        async_worker.run().await;
      });
    }

    drop(tx);

    while let Some(worker_message) = rx.recv().await {
      use WorkerMessage::*;
      debug!("[AsyncWorker] Received new message {:?}", worker_message);
      match &worker_message {
        NewDependency(resolved_id) => {
          modules_to_work.lock().unwrap().push(resolved_id.to_owned());
        }
      }
      println!("Got message = {:?}", worker_message);
    }

    Ok(())
  }
}
