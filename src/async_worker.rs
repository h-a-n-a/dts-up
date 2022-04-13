use std::path::Path;
use std::sync::{Arc, Mutex};

use dashmap::DashSet;
use smol_str::SmolStr;
use tokio::sync::mpsc::Sender;

use crate::ast;
use crate::result::Error;
use crate::utils::resolve_id;

#[derive(Debug)]
pub enum WorkerMessage {
  NewModule(ast::module::Module),
  // NewDependency()
}

#[derive(Debug)]
pub struct AsyncWorker {
  pub resp_tx: Sender<WorkerMessage>,
  pub modules_to_work: Arc<Mutex<Vec<SmolStr>>>,
  pub worked_modules: Arc<DashSet<SmolStr>>,
  pub resolved_entries: Arc<DashSet<SmolStr>>,
}

impl AsyncWorker {
  fn fetch_job(&mut self) -> Option<SmolStr> {
    while let Some(resolved_id) = self.modules_to_work.lock().unwrap().pop() {
      if !self.worked_modules.contains(&resolved_id) {
        self.worked_modules.insert(resolved_id.clone());
        return Some(resolved_id.clone());
      }
    }

    None
  }

  fn discover_module(&mut self, module: &ast::module::Module) {
    let sub_modules = module.pre_analyze_sub_modules();

    sub_modules.iter().for_each(|module_id| {
      let module_id = resolve_id(
        nodejs_path::resolve!(
          nodejs_path::dirname(module.id.as_str()),
          module_id.to_string()
        )
        .as_str(),
      );
      self.modules_to_work.lock().unwrap().push(module_id);
    })
  }

  pub async fn run(&mut self) {
    use ast::*;

    if let Some(resolved_id) = self.fetch_job() {
      log::debug!("[AsyncWorker]: running job {}", resolved_id);
      let swc_module = ast::parse::parse_file(resolved_id.clone()).await.unwrap();

      let mut module = module::Module::from_swc_module(module::ModuleOptions {
        id: resolved_id.clone(),
        is_entry: self.resolved_entries.contains(&resolved_id),
        swc_module,
      });

      self.discover_module(&module);

      self.resp_tx.send(WorkerMessage::NewModule(module)).await;
    }
  }
}
