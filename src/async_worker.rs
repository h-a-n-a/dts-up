use std::path::Path;
use std::sync::{Arc, Mutex};

use dashmap::DashSet;
use smol_str::SmolStr;
use tokio::sync::mpsc::Sender;

use crate::ast;
use crate::result::Error;

#[derive(Debug)]
pub enum WorkerMessage {
  NewDependency(SmolStr),
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

  pub async fn run(&mut self) {
    use ast::*;

    if let Some(resolved_id) = self.fetch_job() {
      let swc_module = ast::parse::parse_file(resolved_id.clone()).await.unwrap();

      let mut module = module::Module::from_swc_module(module::ModuleOptions {
        id: resolved_id.clone(),
        is_entry: self.resolved_entries.contains(&resolved_id),
        swc_module,
      });

      let imports = module.pre_analyze_import_decl();

      crate::utils::resolve_dts!("/User", "./a");

      println!("{:?}", imports);

      let path = Path::new(module.id.as_str());

      for module_id in imports {
        let module_id = module_id.as_str();

        // TODO: external module
        let _ = self
          .resp_tx
          .send(WorkerMessage::NewDependency(SmolStr::new(
            path.join(module_id).to_string_lossy().as_ref(),
          )))
          .await;
      }

      // imports.iter().for_each(|module_id| {});
    }
  }
}
