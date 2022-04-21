use std::collections::{hash_map, HashMap, HashSet};

use swc_atoms::{js_word, JsWord};
use swc_common::Mark;
use swc_ecma_ast::{Decl, ExportSpecifier, Pat, TsType, TsTypeElement};
use swc_ecma_visit::{noop_visit_mut_type, VisitMut, VisitMutWith};

use super::module::{ImportIdent, Source};
use super::{
  scope::{Definition, Scope, ScopeKind, VariableDeclaration},
  symbol::{self, MarkExt, SyntaxContextExt},
  utils::{get_module_export_name, mark_module_export_name},
};

type LocalName = JsWord;

#[derive(Debug, Clone)]
pub enum ExportOriginalIdent {
  // source of export ident, only available when exporting with `export {} from ".."`
  Name(LocalName, Option<Source>),
  Namespace,
  All,
}

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
  pub src: Source,
}

#[derive(Debug, Clone)]
pub struct ModuleExportName {
  pub exported_name: LocalName,
  pub original_ident: ExportOriginalIdent,
  pub mark: Mark,
  // without src: `export { name }`, with src: `export { name } from "./foo"`
  pub src: Option<Source>,
  // index for locating import/export order, only available for situation in which `src` is `some`
  pub index: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ModuleExportNamespace {
  pub index: u32,
  pub exported_name: LocalName,
  pub original_ident: ExportOriginalIdent,
  pub mark: Mark,
  pub src: Source,
}

#[derive(Debug)]
pub struct ModuleExportAll {
  pub index: u32,
  pub src: Source,
}

#[derive(Debug)]
pub enum ModuleExport {
  Name(ModuleExportName),
  Namespace(ModuleExportNamespace),
  All(ModuleExportAll),
}

/// StatementContexts will be used to create real statements
#[derive(Clone, Debug, Default)]
pub struct StatementContext {
  index: u32,

  // In best practices `StatementContext` should be split to enums to avoid problematic `unwraps` for `mark`
  pub is_import: bool,
  pub is_export: bool,
  // IMPORTANT:
  // export declarations like `export declare type Foo = string` or export namespaced are declarations,
  // which is supposed to be transformed(`export declare` -> `declare`, or `export * as foo from "./foo"` -> declare namespace "foo") and generated
  pub is_export_decl: bool,

  pub reads: HashSet<Mark>,

  // Tree-shaking includes statement with its mark
  // `None` if `is_import` or `is_export` equals to `true`
  pub mark: Option<Mark>,
}

#[derive(Debug)]
pub struct ModuleAnalyzer {
  scope: Vec<Scope>,

  current_import_index: u32,
  current_statement_index: u32,

  /// LocalName is always available for imports
  pub imports: HashMap<LocalName, ModuleImport>,
  pub exports: Vec<ModuleExport>,
  pub statement_context: Vec<StatementContext>,
}

impl ModuleAnalyzer {
  pub fn new() -> Self {
    Self {
      scope: vec![Scope::new(ScopeKind::TypeScope)],
      current_import_index: Default::default(),
      current_statement_index: Default::default(),
      imports: Default::default(),
      exports: Default::default(),
      statement_context: Default::default(),
    }
  }

  fn push_scope(&mut self, scope: Scope) {
    self.scope.push(scope);
  }

  fn pop_scope(&mut self) {
    self.scope.pop();
  }

  fn advance_statement(&mut self) {
    self.current_statement_index += 1;
  }

  fn get_current_statement(&self) -> Option<&StatementContext> {
    self
      .statement_context
      .get(self.current_statement_index as usize)
  }

  // It seems not necessary since we've already done this in `add_variable_read`
  // fn sync_current_scope_reads_to_statement(&mut self) {
  //   let scope_reads = self.get_current_scope().unwrap().get_reads().clone();
  //   let ctxt = self.get_current_statement_mut().unwrap();
  //   ctxt.reads.extend(scope_reads);
  // }

  fn get_current_statement_mut(&mut self) -> Option<&mut StatementContext> {
    self
      .statement_context
      .get_mut(self.current_statement_index as usize)
  }

  fn pop_scope_on_type_param(&mut self) {
    if matches!(
      self.get_current_scope_kind(),
      Some(&ScopeKind::TsTypeParameter)
    ) {
      self.pop_scope();
    }
  }

  fn advance_import_index(&mut self) {
    self.current_import_index += 1;
  }

  fn get_current_scope(&self) -> Option<&Scope> {
    self.scope.last()
  }

  fn get_current_scope_mut(&mut self) -> Option<&mut Scope> {
    self.scope.last_mut()
  }

  fn get_current_scope_kind(&self) -> Option<&ScopeKind> {
    self.scope.last().map(|s| &s.kind)
  }

  fn add_variable_read(&mut self, name: &JsWord) -> Option<Mark> {
    if let Some(mark) = self.get_mark_by_name(name) {
      let ctxt = self.get_current_statement_mut().unwrap();
      ctxt.reads.insert(mark.clone());

      let scope = self.get_current_scope_mut().unwrap();
      scope.add_variable_read(mark.clone());

      return Some(mark);
    }

    // Maybe we encountered a global type reference, which is not necessary for us to bind

    None
  }

  fn add_variable_definition(
    &mut self,
    name: JsWord,
    definition_type: VariableDeclaration,
    new_mark: Mark,
  ) {
    let scope = self.get_current_scope_mut().unwrap();
    scope.add_variable_definition(name, definition_type, new_mark);
  }

  fn get_top_level_names(&self) -> Vec<JsWord> {
    let mut top_level_names = self.imports.keys().cloned().collect::<Vec<JsWord>>();

    if let Some(top_level_scope) = self.scope.first() {
      top_level_names.extend(top_level_scope.definitions.keys().cloned());
    }

    top_level_names
  }

  #[inline]
  fn get_mark_by_name(&self, name: &JsWord) -> Option<Mark> {
    // reverse iterate over scopes to find if `name` is available
    for scope in self.scope.iter().rev() {
      if let Some(def) = scope.get_variable_definition(name) {
        return Some(def.mark);
      }
    }

    // or iterate over imports
    self.imports.iter().find_map(|(imported, module_import)| {
      if imported == name {
        Some(module_import.mark.clone())
      } else {
        None
      }
    })
  }

  fn add_import(&mut self, import_decl: &mut swc_ecma_ast::ImportDecl) {
    use swc_ecma_ast::{ImportSpecifier, ModuleExportName};

    let index = self.current_import_index;
    let src = import_decl.src.value.clone();
    let len = import_decl.specifiers.len();
    let mut original_idents: Vec<ImportIdent> = Vec::with_capacity(len);
    let mut local_names: Vec<JsWord> = Vec::with_capacity(len);
    let mut marks: Vec<Mark> = Vec::with_capacity(len);

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

        let new_mark = symbol::new_mark();
        marks.push(new_mark);
        s.span.ctxt = new_mark.as_ctxt();
      }
      ImportSpecifier::Default(s) => {
        local_names.push(s.local.sym.clone());
        original_idents.push(ImportIdent::Name(js_word!("default")));

        let new_mark = symbol::new_mark();
        marks.push(new_mark);
        s.span.ctxt = new_mark.as_ctxt();
      }
      ImportSpecifier::Namespace(s) => {
        local_names.push(s.local.sym.clone());
        original_idents.push(ImportIdent::Namespace);

        let new_mark = symbol::new_mark();
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
  fn visit_mut_module(&mut self, n: &mut swc_ecma_ast::Module) {
    self.statement_context = vec![Default::default(); n.body.len()];
    self
      .statement_context
      .iter_mut()
      .enumerate()
      .for_each(|(index, ctxt)| {
        ctxt.index = index as u32;
      });

    n.visit_mut_children_with(self);
  }

  fn visit_mut_module_item(&mut self, n: &mut swc_ecma_ast::ModuleItem) {
    n.visit_mut_children_with(self);
    self.advance_statement();
  }

  // TODO: support more declarations

  fn visit_mut_fn_decl(&mut self, n: &mut swc_ecma_ast::FnDecl) {
    n.function.type_params.visit_mut_with(self);
    // n.visit_mut_children_with(self);

    self.pop_scope_on_type_param();
  }

  fn visit_mut_class_decl(&mut self, n: &mut swc_ecma_ast::ClassDecl) {
    n.class.type_params.visit_mut_with(self);
  }

  fn visit_mut_ts_type_ann(&mut self, n: &mut swc_ecma_ast::TsTypeAnn) {
    // already included in `TsType` visitor
    n.visit_mut_children_with(self);
  }

  fn visit_mut_ts_type_alias_decl(&mut self, n: &mut swc_ecma_ast::TsTypeAliasDecl) {
    use swc_ecma_ast::TsTypeAliasDecl;

    let new_mark = symbol::new_mark();
    n.id.span.ctxt = new_mark.as_ctxt();

    let ctxt = self.get_current_statement_mut().unwrap();
    ctxt.mark = Some(new_mark.clone());

    self.add_variable_definition(
      n.id.sym.clone(),
      VariableDeclaration::TsTypeAliasDeclaration,
      new_mark,
    );

    n.type_params.visit_mut_with(self);

    self.push_scope(Scope::new(ScopeKind::TypeScope));
    n.type_ann.visit_mut_with(self);
    self.pop_scope();

    self.pop_scope_on_type_param();
  }

  fn visit_mut_ts_type_element(&mut self, n: &mut swc_ecma_ast::TsTypeElement) {
    use swc_ecma_ast::{Expr, TsTypeElement};
    match n {
      TsTypeElement::TsPropertySignature(prop) => {
        prop.type_ann.visit_mut_with(self);
      }
      _ => {
        // temporarily not support
      }
    }
  }

  fn visit_mut_ts_interface_decl(&mut self, n: &mut swc_ecma_ast::TsInterfaceDecl) {
    n.type_params.visit_mut_with(self);

    let new_mark = symbol::new_mark();
    n.id.span.ctxt = new_mark.as_ctxt();

    let ctxt = self.get_current_statement_mut().unwrap();
    ctxt.mark = Some(new_mark.clone());

    let scope = self.get_current_scope_mut().unwrap();
    scope.add_variable_definition(
      n.id.sym.clone(),
      VariableDeclaration::TsInterfaceDeclaration,
      new_mark,
    );

    self.push_scope(Scope::new(ScopeKind::TypeScope));

    n.extends.iter_mut().for_each(|extend| {
      use swc_ecma_ast::Expr;
      match extend.expr.as_mut() {
        Expr::Ident(ident) => {
          let mut mark = self.add_variable_read(&ident.sym);
          if let Some(mark) = mark.as_ref() {
            ident.span.ctxt = mark.as_ctxt();
          }
        }
        _ => {
          // currently we don't support these
        }
      }
    });

    n.body.visit_mut_with(self);

    self.pop_scope();
    self.pop_scope_on_type_param();
  }

  fn visit_mut_ts_type_param_decl(&mut self, n: &mut swc_ecma_ast::TsTypeParamDecl) {
    self.push_scope(Scope::new(ScopeKind::TsTypeParameter));

    if let Some(scope) = self.get_current_scope_mut() {
      n.params.iter_mut().for_each(|param| {
        let new_mark = symbol::new_mark();
        param.span.ctxt = new_mark.as_ctxt();

        scope.add_variable_definition(
          param.name.sym.clone(),
          VariableDeclaration::TsTypeParameter,
          new_mark,
        );

        // `param.default` is visited with `visit_mut_ts_type`
      });
    }

    n.visit_mut_children_with(self);
  }

  fn visit_mut_module_decl(&mut self, n: &mut swc_ecma_ast::ModuleDecl) {
    use swc_ecma_ast::{ImportDecl, ModuleDecl};
    match n {
      ModuleDecl::Import(import_decl) => {
        self.add_import(import_decl);

        let ctxt = self.get_current_statement_mut().unwrap();
        ctxt.is_import = true;

        self.advance_import_index();
      }
      ModuleDecl::ExportDecl(export_decl) => {
        use swc_ecma_ast::{Decl, Pat};

        let ctxt = self.get_current_statement_mut().unwrap();
        ctxt.is_export = true;
        ctxt.is_export_decl = true;

        match &mut export_decl.decl {
          Decl::Var(v) => {
            v.visit_mut_with(self);

            v.decls.iter_mut().for_each(|decl| match &mut decl.name {
              Pat::Ident(ident) => {
                let new_mark = ident.id.span.ctxt.as_mark();

                self.exports.push(ModuleExport::Name(ModuleExportName {
                  exported_name: ident.id.sym.clone(),
                  original_ident: ExportOriginalIdent::Name(ident.id.sym.clone(), None),
                  mark: new_mark,
                  src: None,
                  index: None,
                }));
              }
              p => {
                log::warn!(
                  "[ModuleAnalyzer] Pattern type {:?} should not available in dts file",
                  p
                )
              }
            });
          }
          Decl::Class(c) => {
            c.visit_mut_with(self);
            let new_mark = c.ident.span.ctxt.as_mark();

            self.exports.push(ModuleExport::Name(ModuleExportName {
              exported_name: c.ident.sym.clone(),
              original_ident: ExportOriginalIdent::Name(c.ident.sym.clone(), None),
              mark: new_mark,
              src: None,
              index: None,
            }))
          }
          Decl::Fn(f) => {
            f.visit_mut_with(self);
            let new_mark = f.ident.span.ctxt.as_mark();

            self.exports.push(ModuleExport::Name(ModuleExportName {
              exported_name: f.ident.sym.clone(),
              original_ident: ExportOriginalIdent::Name(f.ident.sym.clone(), None),
              mark: new_mark,
              src: None,
              index: None,
            }))
          }
          Decl::TsInterface(t) => {
            t.visit_mut_with(self);
            let new_mark = t.id.span.ctxt.as_mark();

            self.exports.push(ModuleExport::Name(ModuleExportName {
              exported_name: t.id.sym.clone(),
              original_ident: ExportOriginalIdent::Name(t.id.sym.clone(), None),
              mark: new_mark,
              src: None,
              index: None,
            }))
          }
          Decl::TsTypeAlias(t) => {
            t.visit_mut_with(self);
            let new_mark = t.id.span.ctxt.as_mark();

            self.exports.push(ModuleExport::Name(ModuleExportName {
              exported_name: t.id.sym.clone(),
              original_ident: ExportOriginalIdent::Name(t.id.sym.clone(), None),
              mark: new_mark,
              src: None,
              index: None,
            }))
          }
          Decl::TsEnum(t) => {
            t.visit_mut_with(self);
            let new_mark = t.id.span.ctxt.as_mark();

            self.exports.push(ModuleExport::Name(ModuleExportName {
              exported_name: t.id.sym.clone(),
              original_ident: ExportOriginalIdent::Name(t.id.sym.clone(), None),
              mark: new_mark,
              src: None,
              index: None,
            }))
          }
          Decl::TsModule(t) => {}
        }
      }
      ModuleDecl::ExportNamed(named_export) => {
        use swc_ecma_ast::ExportSpecifier;

        let ctxt = self.get_current_statement_mut().unwrap();
        ctxt.is_export = true;

        let mut is_export_decl = false;

        named_export.specifiers.iter_mut().for_each(|s| match s {
          ExportSpecifier::Named(named) => {
            let new_mark = self
              .get_mark_by_name(&get_module_export_name(&named.orig))
              .unwrap_or_else(|| symbol::new_mark());

            let exported_name: JsWord = {
              use swc_ecma_ast::ModuleExportName;

              let name;
              if named.exported.is_some() {
                mark_module_export_name(named.exported.as_mut().unwrap(), new_mark);
                name = named.exported.as_ref();
              } else {
                mark_module_export_name(&mut named.orig, new_mark);
                name = Some(&named.orig);
              }

              get_module_export_name(name.unwrap())
            };

            let src = named_export.src.as_ref().map(|src| src.value.clone());

            self.exports.push(ModuleExport::Name(ModuleExportName {
              exported_name,
              original_ident: ExportOriginalIdent::Name(
                get_module_export_name(&named.orig),
                src.clone(),
              ),
              mark: new_mark,
              src: src.clone(),
              index: if src.is_some() {
                Some(self.current_import_index)
              } else {
                None
              },
            }));

            if src.is_some() {
              self.advance_import_index();
            }
          }
          ExportSpecifier::Namespace(namespace) => {
            let new_mark = symbol::new_mark();
            namespace.span.ctxt = new_mark.as_ctxt();
            is_export_decl = true;

            self
              .exports
              .push(ModuleExport::Namespace(ModuleExportNamespace {
                exported_name: get_module_export_name(&namespace.name),
                original_ident: ExportOriginalIdent::Namespace,
                mark: new_mark,
                // source is always available in namespaces
                src: named_export
                  .src
                  .as_ref()
                  .map(|src| src.value.clone())
                  .unwrap(),
                index: self.current_import_index,
              }));

            self.advance_import_index();
          }
          ExportSpecifier::Default(default) => {}
        });

        let ctxt = self.get_current_statement_mut().unwrap();
        ctxt.is_export_decl = is_export_decl;

        n.visit_mut_children_with(self);
      }
      ModuleDecl::ExportDefaultDecl(export_default) => {
        let ctxt = self.get_current_statement_mut().unwrap();
        ctxt.is_export = true;
        ctxt.is_export_decl = true;

        let name = js_word!("default");
        let new_mark = symbol::new_mark();
        export_default.span.ctxt = new_mark.as_ctxt();

        self.exports.push(ModuleExport::Name(ModuleExportName {
          exported_name: name.clone(),
          original_ident: ExportOriginalIdent::Name(name.clone(), None),
          mark: new_mark,
          src: None,
          index: None,
        }));

        n.visit_mut_children_with(self);
      }
      ModuleDecl::ExportAll(export_all) => {
        let ctxt = self.get_current_statement_mut().unwrap();
        ctxt.is_export = true;

        let new_mark = symbol::new_mark();
        export_all.span.ctxt = new_mark.as_ctxt();

        self.exports.push(ModuleExport::All(ModuleExportAll {
          src: export_all.src.value.clone(),
          index: self.current_import_index,
        }));
        self.advance_import_index();

        n.visit_mut_children_with(self);
      }
      ModuleDecl::ExportDefaultExpr(export_default_expr) => {
        log::warn!("[ModuleAnalyzer] `ExportDefaultExpr` should not exist in dts files");
      }
      _ => (),
    }
  }

  fn visit_mut_ts_type(&mut self, n: &mut swc_ecma_ast::TsType) {
    use swc_ecma_ast::TsType;
    match n {
      TsType::TsKeywordType(t) => {
        // skip
      }

      TsType::TsThisType(t) => {
        // skip
      }

      TsType::TsFnOrConstructorType(t) => {
        // TODO: is it necessary?
      }

      TsType::TsTypeRef(t) => {
        use swc_ecma_ast::TsEntityName;
        // reference to a TS type
        match &mut t.type_name {
          TsEntityName::Ident(ident) => {
            let mut mark = self.add_variable_read(&ident.sym);
            if let Some(mark) = mark.as_ref() {
              ident.span.ctxt = mark.as_ctxt();
            }
          }
          TsEntityName::TsQualifiedName(q) => {}
        }
      }

      TsType::TsTypeQuery(t) => {}

      TsType::TsTypeLit(t) => {
        self.push_scope(Scope::new(ScopeKind::TypeScope));
        t.visit_mut_children_with(self);
        self.pop_scope();
      }

      TsType::TsArrayType(t) => {
        // recursively visit its children
        t.elem_type.visit_mut_children_with(self);
      }

      TsType::TsTupleType(t) => {}

      TsType::TsOptionalType(t) => {}

      TsType::TsRestType(t) => {}

      TsType::TsUnionOrIntersectionType(t) => {}

      TsType::TsConditionalType(t) => {}

      TsType::TsInferType(t) => {}

      TsType::TsParenthesizedType(t) => {}

      TsType::TsTypeOperator(t) => {}

      TsType::TsIndexedAccessType(t) => {}

      TsType::TsMappedType(t) => {}

      TsType::TsLitType(t) => {}

      TsType::TsTypePredicate(t) => {}

      TsType::TsImportType(t) => {}
    }
  }
}
