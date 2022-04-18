use std::collections::{hash_map, HashMap, HashSet};

use swc_atoms::{js_word, JsWord};
use swc_common::Mark;
use swc_ecma_ast::{Decl, Pat};
use swc_ecma_visit::{noop_visit_mut_type, VisitMut, VisitMutWith};

use super::module::{ExportIdent, ImportIdent};
use super::{
  scope::{Definition, Scope, VariableDeclaration},
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

#[derive(Debug)]
pub struct ModuleExportName {
  pub exported_name: LocalName,
  pub original_ident: ExportIdent,
  pub mark: Mark,
  pub src: Option<JsWord>,
}

#[derive(Debug)]
pub struct ModuleExportNamespace {
  pub exported_name: LocalName,
  pub original_ident: ExportIdent,
  pub mark: Mark,
  pub src: JsWord,
}

#[derive(Debug)]
pub struct ModuleExportAll {
  pub src: JsWord,
}

#[derive(Debug)]
pub enum ModuleExport {
  Name(ModuleExportName),
  Namespace(ModuleExportNamespace),
  All(ModuleExportAll),
}

#[derive(Debug)]
pub struct ModuleAnalyzer {
  pub scope: Vec<Scope>,
  pub current_scope_depth: u32,
  pub current_import_index: u32,
  /// LocalName is always available for imports
  pub imports: HashMap<LocalName, ModuleImport>,
  pub exports: Vec<ModuleExport>,
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
        original_idents.push(ImportIdent::Name(js_word!("default")));

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
}

impl VisitMut for ModuleAnalyzer {
  noop_visit_mut_type!();

  fn visit_mut_module_decl(&mut self, n: &mut swc_ecma_ast::ModuleDecl) {
    use swc_ecma_ast::{ImportDecl, ModuleDecl};
    match n {
      ModuleDecl::Import(import_decl) => {
        self.add_import(import_decl);
        self.advance_import_index();
      }
      ModuleDecl::ExportDecl(export_decl) => {
        use swc_ecma_ast::{Decl, Pat};

        match &export_decl.decl {
          Decl::Var(v) => v.decls.iter().for_each(|decl| match &decl.name {
            Pat::Ident(ident) => {
              let new_mark = symbol::SYMBOL_BOX.lock().unwrap().new_mark();

              self.get_current_scope_mut().and_then(|scope| {
                scope.definitions.insert(
                  ident.id.sym.clone(),
                  Definition::new(new_mark, VariableDeclaration::VariableDeclaration),
                )
              });

              self.exports.push(ModuleExport::Name(ModuleExportName {
                exported_name: ident.id.sym.clone(),
                original_ident: ExportIdent::Name(ident.id.sym.clone(), None),
                mark: new_mark,
                src: None,
              }));
            }
            p => {
              log::warn!(
                "[ModuleAnalyzer] Pattern type {:?} should not available in dts file",
                p
              )
            }
          }),
          Decl::Class(c) => {
            let new_mark = symbol::SYMBOL_BOX.lock().unwrap().new_mark();

            self.get_current_scope_mut().and_then(|scope| {
              scope.definitions.insert(
                c.ident.sym.clone(),
                Definition::new(new_mark, VariableDeclaration::ClassDeclaration),
              )
            });

            self.exports.push(ModuleExport::Name(ModuleExportName {
              exported_name: c.ident.sym.clone(),
              original_ident: ExportIdent::Name(c.ident.sym.clone(), None),
              mark: new_mark,
              src: None,
            }))
          }
          Decl::Fn(f) => {
            let new_mark = symbol::SYMBOL_BOX.lock().unwrap().new_mark();

            self.get_current_scope_mut().and_then(|scope| {
              scope.definitions.insert(
                f.ident.sym.clone(),
                Definition::new(new_mark, VariableDeclaration::FunctionDeclaration),
              )
            });

            self.exports.push(ModuleExport::Name(ModuleExportName {
              exported_name: f.ident.sym.clone(),
              original_ident: ExportIdent::Name(f.ident.sym.clone(), None),
              mark: new_mark,
              src: None,
            }))
          }
          Decl::TsInterface(t) => {
            let new_mark = symbol::SYMBOL_BOX.lock().unwrap().new_mark();

            self.get_current_scope_mut().and_then(|scope| {
              scope.definitions.insert(
                t.id.sym.clone(),
                Definition::new(new_mark, VariableDeclaration::TsInterfaceDeclaration),
              )
            });

            self.exports.push(ModuleExport::Name(ModuleExportName {
              exported_name: t.id.sym.clone(),
              original_ident: ExportIdent::Name(t.id.sym.clone(), None),
              mark: new_mark,
              src: None,
            }))
          }
          Decl::TsTypeAlias(t) => {
            let new_mark = symbol::SYMBOL_BOX.lock().unwrap().new_mark();

            self.get_current_scope_mut().and_then(|scope| {
              scope.definitions.insert(
                t.id.sym.clone(),
                Definition::new(new_mark, VariableDeclaration::TsTypeAliasDeclaration),
              )
            });

            self.exports.push(ModuleExport::Name(ModuleExportName {
              exported_name: t.id.sym.clone(),
              original_ident: ExportIdent::Name(t.id.sym.clone(), None),
              mark: new_mark,
              src: None,
            }))
          }
          Decl::TsEnum(t) => {
            let new_mark = symbol::SYMBOL_BOX.lock().unwrap().new_mark();

            self.get_current_scope_mut().and_then(|scope| {
              scope.definitions.insert(
                t.id.sym.clone(),
                Definition::new(new_mark, VariableDeclaration::TsEnumDeclaration),
              )
            });

            self.exports.push(ModuleExport::Name(ModuleExportName {
              exported_name: t.id.sym.clone(),
              original_ident: ExportIdent::Name(t.id.sym.clone(), None),
              mark: new_mark,
              src: None,
            }))
          }
          Decl::TsModule(t) => {}
        }
      }
      ModuleDecl::ExportNamed(named_export) => {}
      ModuleDecl::ExportDefaultDecl(ExportDefaultDecl) => {}
      ModuleDecl::ExportDefaultExpr(ExportDefaultExpr) => {}
      ModuleDecl::ExportAll(ExportAll) => {}
      _ => (),
    }
  }
}
