use std::collections::HashMap;
use swc_common::Mark;

use swc_common::util::take::Take;
use swc_ecma_visit::{noop_fold_type, Fold};

use crate::ast::module::{Exports, LocalName};
use crate::ast::symbol::MarkExt;

pub struct Finalizer {
  pub(crate) top_level_exports: HashMap<LocalName, Exports>,
}

impl Finalizer {
  pub fn generate_exports(&self) -> swc_ecma_ast::ModuleItem {
    use swc_ecma_ast::{
      ExportNamedSpecifier, ExportSpecifier, Ident, ModuleDecl, ModuleExportName, ModuleItem,
      NamedExport,
    };

    ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(NamedExport {
      span: Default::default(),
      specifiers: self
        .top_level_exports
        .values()
        .filter_map(|export| {
          let mut exported_name;
          let mut original_name;
          let mut mark;
          match export {
            Exports::Name(e) => {
              exported_name = e.exported_name.clone();
              original_name = e.original_ident.clone();
              mark = e.mark;
            }
            Exports::Namespace(e) => {
              // currently not supported
              return None;
            }
          };

          Some(ExportSpecifier::Named(ExportNamedSpecifier {
            span: Default::default(),
            orig: ModuleExportName::Ident(Ident {
              sym: original_name.clone(),
              ..Ident::dummy()
            }),
            exported: Some(ModuleExportName::Ident(Ident {
              sym: exported_name.clone(),
              ..Ident::dummy()
            })),
            is_type_only: false,
          }))
        })
        .collect::<Vec<_>>(),
      src: None,
      type_only: false,
      asserts: None,
    }))
  }
}

impl Fold for Finalizer {
  noop_fold_type!();

  fn fold_module_item(
    &mut self,
    module_item: swc_ecma_ast::ModuleItem,
  ) -> swc_ecma_ast::ModuleItem {
    use swc_ecma_ast::{Decl, DefaultDecl, ModuleDecl, ModuleItem, Stmt};

    let result = match module_item {
      ModuleItem::ModuleDecl(s) => {
        log::debug!("[Finalizer] folding ModuleDecl {:?}", s);

        match s {
          ModuleDecl::ExportDecl(export_decl) => ModuleItem::Stmt(Stmt::Decl(export_decl.decl)),
          ModuleDecl::ExportNamed(named) => ModuleItem::dummy(),
          ModuleDecl::ExportDefaultDecl(decl) => match decl.decl {
            DefaultDecl::TsInterfaceDecl(ts_interface) => {
              ModuleItem::Stmt(Stmt::Decl(Decl::TsInterface(ts_interface)))
            }
            _ => ModuleItem::dummy(),
          },
          ModuleDecl::ExportDefaultExpr(expr) => ModuleItem::dummy(),
          ModuleDecl::Import(import_decl) => ModuleItem::dummy(),
          ModuleDecl::ExportAll(_) => ModuleItem::dummy(),
          ModuleDecl::TsImportEquals(_) => ModuleItem::dummy(),
          ModuleDecl::TsExportAssignment(_) => ModuleItem::dummy(),
          ModuleDecl::TsNamespaceExport(_) => ModuleItem::dummy(),
        }
      }
      ModuleItem::Stmt(_) => module_item,
    };

    result
  }
}
