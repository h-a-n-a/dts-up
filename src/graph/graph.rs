use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::{atomic::AtomicUsize, Arc, Mutex};

use dashmap::DashSet;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use smol_str::SmolStr;
use tokio::sync::mpsc::{self, Sender};

use crate::{
  ast::{self, module::ModuleId},
  graph::{
    async_worker::{AsyncWorker, WorkerMessage},
    ModuleEdge, ModuleGraph, ModuleIndex,
  },
  result::Error,
  utils::resolve_id,
};

#[derive(Debug)]
pub struct Graph {
  resolved_entry: ModuleId,
  entry_module_index: ModuleIndex,
  sorted_modules: Vec<ModuleIndex>,
  module_graph: ModuleGraph,
  id_to_module: HashMap<ModuleId, ast::module::Module>,
}

#[derive(Debug)]
pub struct GraphOptions<T: AsRef<str>> {
  pub entry: T,
}

impl Graph {
  pub fn new<T>(options: GraphOptions<T>) -> Self
  where
    T: AsRef<str>,
  {
    let resolved_entry = resolve_id(&nodejs_path::resolve!(options.entry.as_ref()));

    Self {
      resolved_entry,
      sorted_modules: Default::default(),
      entry_module_index: Default::default(),
      id_to_module: Default::default(),
      module_graph: ModuleGraph::new(),
    }
  }

  pub async fn build(&mut self) -> Result<(), Error> {
    self.generate().await?;
    self.sort_modules();

    Ok(())
  }

  pub async fn generate(&mut self) -> Result<(), Error> {
    let num_of_threads = num_cpus::get_physical();
    let idle_thread_count: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(num_of_threads));

    let (tx, mut rx) = mpsc::channel::<WorkerMessage>(32);

    // TODO: replace with RwLock
    let modules_to_work: Arc<Mutex<Vec<ModuleId>>> =
      Arc::new(Mutex::new(vec![self.resolved_entry.clone()]));

    self.module_graph.add_module(self.resolved_entry.clone());

    let worked_modules: Arc<DashSet<ModuleId>> = Arc::new(DashSet::new());

    for _ in 0..num_of_threads {
      let idle_thread_count = idle_thread_count.clone();
      let mut async_worker = AsyncWorker {
        resp_tx: tx.clone(),
        modules_to_work: modules_to_work.clone(),
        worked_modules: worked_modules.clone(),
        resolved_entries: Arc::new(DashSet::from_iter(vec![self.resolved_entry.clone()])),
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

    while !modules_to_work.lock().unwrap().is_empty()
      || idle_thread_count.load(Ordering::SeqCst) != num_of_threads
    {
      if let Ok(worker_message) = rx.try_recv() {
        use WorkerMessage::*;
        log::debug!("[AsyncWorker] Received new message -> {}", worker_message);
        match worker_message {
          NewModule(module) => {
            let id = module.id.clone();
            self.id_to_module.insert(id.clone(), module);
            let module_index = self.module_graph.get_or_add_module(id.clone());

            if id == self.resolved_entry {
              self.entry_module_index = module_index;
            }
          }
          NewDependency(from_id, to_id, edge) => {
            let from_module_index = self.module_graph.get_or_add_module(from_id);
            let to_module_index = self.module_graph.get_or_add_module(to_id);
            self
              .module_graph
              .add_edge(from_module_index, to_module_index, edge);
          }
        }
      }
    }

    Ok(())
  }

  pub fn sort_modules(&mut self) {
    let mut sorted: Vec<ModuleIndex> = Default::default();
    let mut stack = vec![self.entry_module_index];
    let mut visited: HashSet<ModuleIndex> = Default::default();

    while let Some(node_index) = stack.pop() {
      if visited.contains(&node_index) {
        sorted.push(node_index);
        continue;
      }

      stack.push(node_index);
      visited.insert(node_index);

      let mut level_edges = self
        .module_graph
        .get_edges_directed(node_index, Direction::Outgoing)
        .filter_map(|edge| {
          let target_module_index = edge.target();
          let weight = edge.weight();

          if visited.contains(&target_module_index) {
            return None;
          }

          if let ModuleEdge::Import(module_import) = weight {
            return Some((target_module_index, module_import.index));
          }

          None
        })
        .collect::<Vec<_>>();

      level_edges.sort_by_key(|e| e.1);

      level_edges
        .iter()
        .for_each(|(module_index, _)| stack.push(module_index.clone()))
    }

    self.sorted_modules = sorted;
  }
}
