use std::collections::{hash_map, HashMap, HashSet};

use swc_atoms::JsWord;
use swc_common::Mark;
use swc_ecma_visit::{noop_visit_mut_type, VisitMut, VisitMutWith};

use super::module::{ExportIdent, ImportIdent};
use super::{
  scope::Scope,
  symbol::{self, MarkExt},
};

type LocalName = JsWord;

#[derive(Debug)]
pub struct ModuleImport {
  /// Index of the same `src` shares the same index
  pub index: u32,
  /// Mark of import variable
  pub mark: Mark,
  /// Identifier's name in current module, `b` in `{ a as b }`
  pub local_name: LocalName,
  /// `a` in `{ a as b }`
  pub original_ident: ImportIdent,
  /// Source(of importee) may be the same since we will split single statement with multiple variables into different imports for **tree-shaking**
  pub src: JsWord,
}

pub struct ModuleExport {
  pub mark: Option<Mark>,

  pub local_name: LocalName,

  pub original_ident: ExportIdent,
}

#[derive(Debug)]
pub struct ModuleAnalyzer {
  pub scope: Vec<Scope>,
  pub current_scope_depth: u32,
  pub current_import_index: u32,
  /// LocalName is always available for imports
  pub imports: HashMap<LocalName, ModuleImport>,
  pub exports: Vec<String>,
}

impl ModuleAnalyzer {
  pub fn new() -> Self {
    Self {
      scope: Default::default(),
      current_scope_depth: Default::default(),
      current_import_index: Default::default(),
      imports: Default::default(),
      exports: Default::default(),
    }
  }

  pub fn advance_import_index(&mut self) {
    self.current_import_index += 1;
  }

  pub fn advance_scope(&mut self) {
    if self.current_scope_depth >= self.scope.len() as u32 {
      panic!(
        "[ModuleAnalyzer:Scope]: failed to advance scope, current depth: {}, current scope length: {}",
        self.current_scope_depth,
        self.scope.len()
      );
    } else {
      self.current_scope_depth += 1;
    }
  }

  pub fn backtrack_scope(&mut self) {
    if self.current_scope_depth > 0 {
      self.current_scope_depth -= 1;
    } else {
      panic!(
        "[ModuleAnalyzer:Scope]: failed to backtrack scope, current depth: {}, current scope length: {}",
        self.current_scope_depth,
        self.scope.len()
      );
    }
  }

  pub fn get_current_scope(&self) -> Option<&Scope> {
    self.scope.get(self.current_scope_depth as usize)
  }

  pub fn get_current_scope_mut(&mut self) -> Option<&mut Scope> {
    self.scope.get_mut(self.current_scope_depth as usize)
  }

  pub fn get_top_level_names(&self) -> Vec<JsWord> {
    let mut top_level_names = self.imports.keys().cloned().collect::<Vec<JsWord>>();

    if let Some(top_level_scope) = self.scope.first() {
      top_level_names.extend(top_level_scope.definitions.keys().cloned());
    }

    top_level_names
  }

  pub fn add_import(&mut self, import_decl: &mut swc_ecma_ast::ImportDecl) {
    use swc_ecma_ast::{ImportSpecifier, ModuleExportName};

    let index = self.current_import_index;
    let src = import_decl.src.value.clone();
    let mut original_idents: Vec<ImportIdent> = Vec::with_capacity(import_decl.specifiers.len());
    let mut local_names: Vec<JsWord> = Vec::with_capacity(import_decl.specifiers.len());
    let mut marks: Vec<Mark> = Vec::with_capacity(import_decl.specifiers.len());

    import_decl.specifiers.iter_mut().for_each(|s| match s {
      ImportSpecifier::Named(s) => {
        local_names.push(s.local.sym.clone());

        // import { foo as bar } from "./foo", imported: `foo`, local: `bar`
        if let Some(n) = &s.imported {
          match n {
            ModuleExportName::Ident(i) => {
              original_idents.push(ImportIdent::Name(i.sym.clone()));
            }
            ModuleExportName::Str(_) => {
              // https://tc39.es/ecma262/#prod-ModuleExportName
            }
          }
        } else {
          // import { foo } from "./foo", imported: None, local: `foo`
          original_idents.push(ImportIdent::Name(s.local.sym.clone()))
        }

        let new_mark = symbol::SYMBOL_BOX.lock().unwrap().new_mark();
        marks.push(new_mark);
        s.span.ctxt = new_mark.as_ctxt();
      }
      ImportSpecifier::Default(s) => {
        local_names.push(s.local.sym.clone());
        original_idents.push(ImportIdent::Default);

        let new_mark = symbol::SYMBOL_BOX.lock().unwrap().new_mark();
        marks.push(new_mark);
        s.span.ctxt = new_mark.as_ctxt();
      }
      ImportSpecifier::Namespace(s) => {
        local_names.push(s.local.sym.clone());
        original_idents.push(ImportIdent::Namespace);

        let new_mark = symbol::SYMBOL_BOX.lock().unwrap().new_mark();
        marks.push(new_mark);
        s.span.ctxt = new_mark.as_ctxt();
      }
    });

    local_names
      .into_iter()
      .zip(original_idents)
      .zip(marks)
      .for_each(|((local_name, original_ident), mark)| {
        if let hash_map::Entry::Vacant(entry) = self.imports.entry(local_name.clone()) {
          let module_import = ModuleImport {
            index,
            mark,
            local_name,
            original_ident,
            src: src.clone(),
          };
          entry.insert(module_import);
        }
      });
  }

  // fn add_export_all
}

impl VisitMut for ModuleAnalyzer {
  noop_visit_mut_type!();

  // fn visit_mut_import_decl(&mut self, n: &mut swc_ecma_ast::ImportDecl) {
  //   self.add_import(n);
  //   self.advance_import_index();
  // }
  //
  // fn visit_mut_export_all(&mut self, n: &mut swc_ecma_ast::ExportAll) {}
  //
  // fn visit_mut_export_decl(&mut self, n: &mut swc_ecma_ast::ExportDecl) {}
  //
  // fn visit_mut_export_named_specifier(&mut self, n: &mut swc_ecma_ast::ExportNamedSpecifier) {}
  //
  // fn visit_mut_export_namespace_specifier(
  //   &mut self,
  //   n: &mut swc_ecma_ast::ExportNamespaceSpecifier,
  // ) {
  // }

  fn visit_mut_module_decl(&mut self, n: &mut swc_ecma_ast::ModuleDecl) {
    use swc_ecma_ast::{ImportDecl, ModuleDecl};
    // match n {
    //   ModuleDecl::Import(import_decl) => {
    //     self.add_import(import_decl);
    //     self.advance_import_index();
    //   }
    // }
  }
}
