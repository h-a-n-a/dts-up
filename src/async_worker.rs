use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::sync::{Arc, Mutex};

use dashmap::DashSet;
use smol_str::SmolStr;
use swc_atoms::JsWord;
use tokio::sync::mpsc::Sender;

use crate::ast::module::{LocalName, ModuleId};
use crate::ast::{
  self,
  module_analyzer::{ModuleExport, ModuleImport},
};
use crate::graph::{ModuleEdge, ModuleEdgeImport};
use crate::result::Error;
use crate::utils::resolve_id;

type FromModule = ModuleId;
type ToModule = ModuleId;

#[derive(Debug)]
pub enum WorkerMessage {
  NewModule(ast::module::Module),
  NewDependency(FromModule, ToModule, ModuleEdge),
}

impl Display for WorkerMessage {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let message = match self {
      WorkerMessage::NewModule(module) => {
        format!("New Module: {}", module.id)
      }
      WorkerMessage::NewDependency(from_id, to_id, edge) => {
        format!(
          "New Dependency from {} to {}, with edge {:?}",
          from_id, to_id, edge
        )
      }
    };
    f.write_str(&message)
  }
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

  fn discover_module(&mut self, module: &mut ast::module::Module) {
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

  pub async fn add_import_graph(
    &mut self,
    module: &ast::module::Module,
    imports: &HashMap<LocalName, ModuleImport>,
  ) {
    let mut import: HashSet<ModuleId> = Default::default();

    for module_import in imports.values() {
      let module_id = module.src_to_resolved_id.get(&module_import.src).unwrap();
      if !import.contains(module_id) {
        import.insert(module_id.clone());
        self
          .resp_tx
          .send(WorkerMessage::NewDependency(
            module.id.clone(),
            module_id.clone(),
            ModuleEdge::Import(ModuleEdgeImport {
              index: module_import.index,
            }),
          ))
          .await;
      }
    }
  }

  pub async fn add_export_graph(
    &mut self,
    module: &ast::module::Module,
    exports: &Vec<ModuleExport>,
  ) {
    for module_export in exports {
      let mut src: Option<JsWord> = Default::default();

      match module_export {
        ModuleExport::Name(e) => {
          src = e.src.clone();
        }
        ModuleExport::All(e) => src = Some(e.src.clone()),
        ModuleExport::Namespace(e) => src = Some(e.src.clone()),
      };

      if let Some(src) = src.take() {
        let resolved_id = module.src_to_resolved_id.get(&src).unwrap();

        self
          .resp_tx
          .send(WorkerMessage::NewDependency(
            module.id.clone(),
            resolved_id.clone(),
            ModuleEdge::Export,
          ))
          .await;
      }
    }
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

      self.discover_module(&mut module);

      let module_analyzer = module.analyze();

      self
        .add_import_graph(&module, &module_analyzer.imports)
        .await;

      self
        .add_export_graph(&module, &module_analyzer.exports)
        .await;

      self.resp_tx.send(WorkerMessage::NewModule(module)).await;
    }
  }
}
