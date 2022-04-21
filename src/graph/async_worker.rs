use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::sync::{Arc, Mutex};

use dashmap::DashSet;
use smol_str::SmolStr;
use swc_atoms::JsWord;
use tokio::sync::mpsc::Sender;

use crate::ast::module::{Exports, LocalName, ModuleId};
use crate::ast::module_analyzer::{ExportOriginalIdent, ModuleExportName};
use crate::ast::{
  self,
  module_analyzer::{ModuleExport, ModuleImport},
};
use crate::graph::{
  ModuleEdge, ModuleEdgeExportAll, ModuleEdgeExportNamed, ModuleEdgeExportNamespace,
  ModuleEdgeImport,
};
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

  fn discover_module(
    &mut self,
    module: &mut ast::module::Module,
    swc_module: &swc_ecma_ast::Module,
  ) {
    let sub_modules = module.pre_analyze_sub_modules(swc_module);

    log::debug!(
      "[AsyncWorker] discovered submodules from {}: {:?}",
      module.id,
      sub_modules
    );

    sub_modules.iter().for_each(|module_id| {
      self.modules_to_work.lock().unwrap().push(module_id.clone());
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
          .await
          .unwrap();
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
      let mut module_export_index: Option<u32>;

      match module_export {
        ModuleExport::Name(e) => {
          src = e.src.clone();
          module_export_index = e.index.clone();
        }
        ModuleExport::All(e) => {
          src = Some(e.src.clone());
          module_export_index = Some(e.index);
        }
        ModuleExport::Namespace(e) => {
          src = Some(e.src.clone());
          module_export_index = Some(e.index);
        }
      };

      if let Some(src) = src.take() {
        let resolved_id = module.src_to_resolved_id.get(&src).unwrap().clone();
        let index = module_export_index.unwrap();

        let module_edge = match module_export {
          ModuleExport::Name(_) => ModuleEdge::ExportNamed(ModuleEdgeExportNamed { index }),
          ModuleExport::All(_) => ModuleEdge::ExportAll(ModuleEdgeExportAll { index }),
          ModuleExport::Namespace(_) => {
            ModuleEdge::ExportNamespace(ModuleEdgeExportNamespace { index })
          }
        };

        self
          .resp_tx
          .send(WorkerMessage::NewDependency(
            module.id.clone(),
            resolved_id,
            module_edge,
          ))
          .await
          .unwrap();
      }
    }
  }

  pub async fn run(&mut self) {
    use ast::*;

    if let Some(resolved_id) = self.fetch_job() {
      log::debug!("[AsyncWorker]: running job {}", resolved_id);
      let mut swc_module = ast::parse::parse_file(resolved_id.clone()).await.unwrap();

      let mut module = module::Module::new(module::ModuleOptions {
        id: resolved_id.clone(),
        is_entry: self.resolved_entries.contains(&resolved_id),
      });

      self.discover_module(&mut module, &swc_module);

      let module_analyzer = module.analyze(&mut swc_module);
      module.generate_statements_from_ctxt(swc_module, module_analyzer.statement_context);

      self
        .add_import_graph(&module, &module_analyzer.imports)
        .await;

      self
        .add_export_graph(&module, &module_analyzer.exports)
        .await;

      module_analyzer
        .exports
        .iter()
        .for_each(|module_export| match module_export {
          ModuleExport::Name(n) => {
            module
              .exports
              .insert(n.exported_name.clone(), Exports::Name(n.clone()));
          }
          ModuleExport::Namespace(n) => {
            module
              .exports
              .insert(n.exported_name.clone(), Exports::Namespace(n.clone()));
          }
          ModuleExport::All(n) => {
            // `export *`s are linked later
          }
        });
      module.local_exports = module_analyzer.exports;

      self
        .resp_tx
        .send(WorkerMessage::NewModule(module))
        .await
        .unwrap();
    }
  }
}
