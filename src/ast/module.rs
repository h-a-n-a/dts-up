use std::collections::HashMap;

use dashmap::DashSet;
use smol_str::SmolStr;
use swc_atoms::JsWord;
use swc_common::Mark;
use swc_ecma_ast::{ImportSpecifier, ModuleDecl, ModuleItem, TsModuleRef};

use super::statement::Statement;
use crate::utils::resolve_id;

pub type ModuleId = SmolStr;

#[derive(Debug)]
pub struct ExportDecl {
  /// Mark of statement
  mark: Mark,
  /// statement_index in a module, if `ExportIdent` is `ExportIdent::Namespace`, then `statement_index` is `None`
  statement_index: Option<usize>,
}

#[derive(Debug)]
pub enum ExportIdent {
  Identifier(JsWord),
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
      exports: Default::default(),
    }
  }

  pub fn pre_analyze_sub_modules(&self) -> DashSet<SmolStr> {
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
        discovered_import.insert(source.into());
      }
    });

    discovered_import
  }

  // fn analyze_module_decl(&mut self) {
  //   use swc_ecma_ast::ModuleDecl;
  //
  //   self
  //     .swc_module
  //     .body
  //     .iter()
  //     .for_each(|module_item| match module_item {
  //       ModuleItem::ModuleDecl(module_decl) => match module_decl {
  //         ModuleDecl::ExportDefaultDecl(export_decl) => {}
  //         ModuleDecl::ExportDecl(export_decl) => {}
  //         ModuleDecl::ExportDefaultExpr(export_expr) => {}
  //         ModuleDecl::ExportNamed(export_named) => {}
  //         ModuleDecl::ExportAll(export_all) => {}
  //         ModuleDecl::TsExportAssignment(ts_export_assign) => {}
  //         ModuleDecl::TsNamespaceExport(ts_namespace_export_decl) => {}
  //         _ => (),
  //       },
  //       _ => {}
  //     })
  // }
  //
  // fn analyze_stmt_decl(&mut self, stmt: &swc_ecma_ast::Stmt) {
  //   use swc_ecma_ast::{ModuleItem, Stmt};
  //
  //   self
  //     .swc_module
  //     .body
  //     .iter()
  //     .for_each(|module_item| match module_item {
  //       ModuleItem::Stmt(stmt) => {
  //         match stmt {
  //           Stmt::Decl(_) => {}
  //           Stmt::Expr(_) => {}
  //           _ => (), // for dts-up, it's not necessary to check other statements
  //         }
  //       }
  //       _ => (),
  //     })
  // }
}
