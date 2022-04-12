use dashmap::DashSet;
use smol_str::SmolStr;
use swc_ecma_ast::{ImportSpecifier, ModuleDecl, ModuleItem};

use super::statement::Statement;
use crate::utils::{resolve_dts, resolve_id};

#[derive(Debug)]
pub struct Module {
  pub id: SmolStr,
  pub is_entry: bool,
  pub statements: Vec<Statement>,
  pub swc_module: swc_ecma_ast::Module,
}

pub struct ModuleOptions {
  pub swc_module: swc_ecma_ast::Module,
  pub id: SmolStr,
  pub is_entry: bool,
}

impl Module {
  pub fn from_swc_module(options: ModuleOptions) -> Self {
    Self {
      swc_module: options.swc_module,
      id: options.id,
      is_entry: options.is_entry,
      statements: Default::default(),
    }
  }

  pub fn pre_analyze_import_decl(&mut self) -> DashSet<SmolStr> {
    use swc_ecma_ast::ModuleDecl;

    let mut discovered_import: DashSet<SmolStr> = DashSet::new();
    let a = vec![];

    self
      .swc_module
      .body
      .iter()
      .for_each(|module_item| match module_item {
        ModuleItem::ModuleDecl(module_decl) => match module_decl {
          ModuleDecl::Import(import_decl) => {
            let resolved_id = resolve_id(import_decl.src.value.clone().into());
            // discovered_import.insert(import_decl.src.value.clone().into());
          }
          ModuleDecl::TsImportEquals(ts_import_equals_decl) => {
            todo!()
          }
          _ => (),
        },
        _ => {}
      });

    discovered_import
  }

  fn analyze_module_decl(&mut self) {
    use swc_ecma_ast::ModuleDecl;

    self
      .swc_module
      .body
      .iter()
      .for_each(|module_item| match module_item {
        ModuleItem::ModuleDecl(module_decl) => match module_decl {
          ModuleDecl::ExportDefaultDecl(export_decl) => {}
          ModuleDecl::ExportDecl(export_decl) => {}
          ModuleDecl::ExportDefaultExpr(export_expr) => {}
          ModuleDecl::ExportNamed(export_named) => {}
          ModuleDecl::ExportAll(export_all) => {}
          ModuleDecl::TsExportAssignment(ts_export_assign) => {}
          ModuleDecl::TsNamespaceExport(ts_namespace_export_decl) => {}
          _ => (),
        },
        _ => {}
      })
  }

  fn analyze_stmt_decl(&mut self, stmt: &swc_ecma_ast::Stmt) {
    use swc_ecma_ast::{ModuleItem, Stmt};

    self
      .swc_module
      .body
      .iter()
      .for_each(|module_item| match module_item {
        ModuleItem::Stmt(stmt) => {
          match stmt {
            Stmt::Decl(_) => {}
            Stmt::Expr(_) => {}
            _ => (), // for dts-up, it's not necessary to check other statements
          }
        }
        _ => (),
      })
  }
}
