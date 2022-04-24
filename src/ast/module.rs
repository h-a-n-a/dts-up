use std::collections::{HashMap, HashSet};

use rayon::prelude::*;
use smol_str::SmolStr;
use swc_atoms::JsWord;
use swc_common::Mark;
use swc_ecma_ast::{ImportSpecifier, ModuleDecl, ModuleItem, TsModuleRef};
use swc_ecma_visit::VisitMutWith;

use super::{
  module_analyzer::{
    ModuleAnalyzer, ModuleExport, ModuleExportName, ModuleExportNamespace, ModuleImport,
    StatementContext,
  },
  statement::Statement,
  symbol,
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

#[derive(Debug, Clone)]
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
  pub imports: HashMap<LocalName, ModuleImport>,
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
      statements: Default::default(),
      imports: Default::default(),
      local_exports: Default::default(),
      src_to_resolved_id: Default::default(),
      exports: Default::default(),
    }
  }

  pub fn pre_analyze_sub_modules(&mut self, swc_module: &swc_ecma_ast::Module) -> HashSet<SmolStr> {
    let mut discovered_import: HashSet<SmolStr> = Default::default();

    let sub_modules = swc_module
      .body
      .iter()
      .filter_map(|module_item| {
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

          return Some((source, resolved_id));
        }

        None
      })
      .collect::<Vec<_>>();

    sub_modules.iter().for_each(|(source, resolved_id)| {
      self
        .src_to_resolved_id
        .entry(source.clone())
        .or_insert_with(|| resolved_id.clone());
      discovered_import.insert(resolved_id.clone());
    });

    discovered_import
  }

  pub fn analyze(&mut self, swc_module: &mut swc_ecma_ast::Module) -> ModuleAnalyzer {
    let mut module_analyzer = ModuleAnalyzer::new();
    swc_module.visit_mut_with(&mut module_analyzer);
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
      .zip(statement_context)
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

  pub fn include_statement_with_mark_set(&mut self, mark_set: &mut HashSet<Mark>) {
    let mut mark_to_local_statement: HashMap<Mark, u32> = Default::default();

    self
      .statements
      .iter()
      .enumerate()
      .for_each(|(index, s)| match s {
        Statement::DeclStatement(s) => {
          mark_to_local_statement.insert(s.mark, index as u32);
        }
        // import statement or non-declarative export statement are omitted
        _ => (),
      });

    let mut maybe_local_reads: Vec<Mark> = Default::default();
    let mut visited: HashSet<Mark> = Default::default();

    self.statements.iter().for_each(|s| match s {
      Statement::DeclStatement(s) => {
        let repr_mark = symbol::SYMBOL_BOX.lock().find_root(s.mark);
        if mark_set.contains(&repr_mark) {
          maybe_local_reads.push(s.mark);
        }
      }
      _ => (),
    });

    while let Some(maybe_local_mark) = maybe_local_reads.pop() {
      if visited.contains(&maybe_local_mark) {
        continue;
      }
      visited.insert(maybe_local_mark);

      // if mark is located in current module, then include it.
      if let Some(index) = mark_to_local_statement.get(&maybe_local_mark) {
        match &mut self.statements[*index as usize] {
          Statement::DeclStatement(s) => {
            log::debug!(
              "[Module] including statement with mark {:?} \nstatement: {:?}",
              s.mark,
              s,
            );
            s.include();
            maybe_local_reads.extend(&s.reads);
          }
          _ => (),
        }
      } else {
        // if not, we find the repr mark, and let it be discovered in later visits.
        let repr_mark = symbol::SYMBOL_BOX.lock().find_root(maybe_local_mark);
        mark_set.insert(repr_mark);
      }
    }
  }
}
