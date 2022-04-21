use std::collections::{HashMap, HashSet};

use smol_str::SmolStr;
use swc_atoms::JsWord;
use swc_common::Mark;
use swc_ecma_ast::{ImportSpecifier, ModuleDecl, ModuleItem, TsModuleRef};
use swc_ecma_visit::VisitMutWith;

use super::{
  module_analyzer::{
    ModuleAnalyzer, ModuleExport, ModuleExportName, ModuleExportNamespace, StatementContext,
  },
  statement::Statement,
};
use crate::utils::resolve_id;

pub type ModuleId = SmolStr;

pub type LocalName = JsWord;
pub type Source = JsWord;

#[derive(Debug)]
pub enum ImportIdent {
  Name(JsWord),
  Namespace,
}

#[derive(Debug)]
pub enum Exports {
  Name(ModuleExportName),
  Namespace(ModuleExportNamespace),
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
  pub local_exports: Vec<ModuleExport>,
  /// In the linking exports process, sub-modules' exports will be synchronized to upper modules
  /// i.e. `export all` in current module will represented as `named exports` here
  ///      `export namespaced` will be kept as is.
  pub exports: HashMap<LocalName, Exports>,

  /// sources(from import or export statement) to avoid resolving a module twice
  pub src_to_resolved_id: HashMap<JsWord, SmolStr>,
}

pub struct ModuleOptions {
  pub id: ModuleId,
  pub is_entry: bool,
}

impl Module {
  pub fn new(options: ModuleOptions) -> Self {
    Self {
      id: options.id,
      is_entry: options.is_entry,
      // Placeholder for type only
      statements: Vec::with_capacity(0),
      local_exports: Default::default(),
      src_to_resolved_id: Default::default(),
      exports: Default::default(),
    }
  }

  pub fn pre_analyze_sub_modules(&mut self, swc_module: &swc_ecma_ast::Module) -> HashSet<SmolStr> {
    let mut discovered_import: HashSet<SmolStr> = Default::default();

    swc_module.body.iter().for_each(|module_item| {
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
            println!("export all {:#?}", export_all);
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

  pub fn analyze(&mut self, swc_module: &mut swc_ecma_ast::Module) -> ModuleAnalyzer {
    let mut module_analyzer = ModuleAnalyzer::new();
    swc_module.visit_mut_with(&mut module_analyzer);

    // println!("{:#?}", module_analyzer);
    module_analyzer
  }

  pub fn generate_statements_from_ctxt(
    &mut self,
    swc_module: swc_ecma_ast::Module,
    statement_context: Vec<StatementContext>,
  ) {
    use super::statement::{DeclStatement, ExportStatementNonDecl, ImportStatement};
    let statements = swc_module
      .body
      .into_iter()
      .zip(statement_context.into_iter())
      .map(|(swc_node, ctxt)| {
        if ctxt.is_import {
          Statement::ImportStatement(ImportStatement::new(swc_node))
        } else if ctxt.is_export && !ctxt.is_export_decl {
          Statement::ExportStatementNonDecl(ExportStatementNonDecl::new(swc_node))
        } else {
          let mut statement = DeclStatement::new(swc_node);
          statement.reads = ctxt.reads;
          statement.is_export_decl = ctxt.is_export_decl;
          statement.mark = ctxt.mark.expect(
            "[Module] `Mark` is supposed to be available in `StatementCtxt`, please file an issue",
          );
          statement.validate_node_type();

          Statement::DeclStatement(statement)
        }
      })
      .collect::<Vec<_>>();

    self.statements = statements;
  }
}
