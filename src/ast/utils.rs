use swc_atoms::JsWord;
use swc_common::Mark;
use swc_ecma_ast::ModuleExportName;

use super::symbol::MarkExt;

pub fn get_module_export_name(name: &ModuleExportName) -> JsWord {
  match name {
    ModuleExportName::Ident(ident) => ident.sym.clone(),
    ModuleExportName::Str(_) => {
      panic!("failed")
    }
  }
}

pub fn mark_module_export_name(name: &mut ModuleExportName, mark: Mark) {
  match name {
    ModuleExportName::Ident(ident) => {
      ident.span.ctxt = mark.as_ctxt();
    }
    ModuleExportName::Str(_) => {}
  }
}
