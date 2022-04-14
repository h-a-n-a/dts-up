use std::collections::HashMap;

use dashmap::DashSet;
use smol_str::SmolStr;
use swc_atoms::JsWord;
use swc_common::Mark;
use swc_ecma_ast::{ImportSpecifier, ModuleDecl, ModuleItem, TsModuleRef};
use swc_ecma_visit::VisitMutWith;

use super::{module_analyzer::ModuleAnalyzer, statement::Statement};
use crate::utils::resolve_id;

pub type ModuleId = SmolStr;

type LocalName = JsWord;
type Source = JsWord;

#[derive(Debug)]
pub struct ExportDecl {
  /// Mark of statement
  mark: Mark,
  /// statement_index in a module, if `ExportIdent` is `ExportIdent::Namespace`, then `statement_index` is `None`
  statement_index: Option<usize>,
}

#[derive(Debug)]
pub enum ExportIdent {
  Name(LocalName, Source),
  Default,
  Namespace,
  All,
}

#[derive(Debug)]
pub enum ImportIdent {
  Name(JsWord),
  Default,
  Namespace,
}

#[derive(Debug)]
pub struct Module {
  /// Absolute Id for module
  pub id: ModuleId,
  /// Is entry module
  pub is_entry: bool,
  /// Raw Statements mapped from
  pub statements: Vec<Statement>,
  /// Local Exports, which does not include sub-modules' exports
  /// 'default', '*'(will only be generated when import namespace is declared from upper modules), and other exports...
  pub local_exports: HashMap<ExportIdent, ExportDecl>,
  /// In the linking exports process, sub-modules' exports will be synchronized to upper modules(nearer to the entry point)
  pub exports: HashMap<ExportIdent, ExportDecl>,

  /// sources(from import or export statement) to avoid resolving a module twice
  pub src_to_resolved_id: HashMap<JsWord, SmolStr>,

  pub swc_module: swc_ecma_ast::Module,
}

pub struct ModuleOptions {
  pub swc_module: swc_ecma_ast::Module,
  pub id: ModuleId,
  pub is_entry: bool,
}

impl Module {
  pub fn from_swc_module(options: ModuleOptions) -> Self {
    Self {
      swc_module: options.swc_module,
      id: options.id,
      is_entry: options.is_entry,
      statements: Default::default(),
      local_exports: Default::default(),
      src_to_resolved_id: Default::default(),
      exports: Default::default(),
    }
  }

  pub fn pre_analyze_sub_modules(&mut self) -> DashSet<SmolStr> {
    let mut discovered_import: DashSet<SmolStr> = DashSet::new();

    self.swc_module.body.iter().for_each(|module_item| {
      let mut discovered: Option<_> = None;
      match module_item {
        ModuleItem::ModuleDecl(module_decl) => match module_decl {
          ModuleDecl::Import(import_decl) => {
            discovered = Some(import_decl.src.value.clone());
          }
          ModuleDecl::ExportNamed(export_named) => {
            if let Some(src) = &export_named.src {
              discovered = Some(src.value.clone());
            }
          }
          ModuleDecl::ExportAll(export_all) => {
            discovered = Some(export_all.src.value.clone());
          }
          ModuleDecl::TsImportEquals(ts_import_decl) => {
            if let TsModuleRef::TsExternalModuleRef(ts_module_ref) = &ts_import_decl.module_ref {
              discovered = Some(ts_module_ref.expr.value.clone());
            }
          }
          _ => (),
        },
        _ => {}
      }

      if let Some(source) = discovered {
        let resolved_id = resolve_id(
          nodejs_path::resolve!(
            nodejs_path::dirname(self.id.as_str()),
            source.as_ref().to_string()
          )
          .as_str(),
        );

        self
          .src_to_resolved_id
          .entry(source)
          .or_insert_with(|| resolved_id.clone());

        discovered_import.insert(resolved_id);
      }
    });

    discovered_import
  }

  pub fn analyze(&mut self) {
    let mut module_analyzer = ModuleAnalyzer::new();
    self.swc_module.visit_mut_with(&mut module_analyzer);
    println!("{:#?}", module_analyzer)
  }
}
