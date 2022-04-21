use std::collections::HashSet;

use swc_common::Mark;
use swc_ecma_ast::{ExportSpecifier, ModuleDecl, ModuleItem, NamedExport};

#[derive(Debug)]
pub enum Statement {
  DeclStatement(DeclStatement),
  ImportStatement(ImportStatement),
  ExportStatementNonDecl(ExportStatementNonDecl),
}

#[derive(Debug)]
pub struct ImportStatement {
  pub node: ModuleItem,
}

impl ImportStatement {
  pub fn new(node: ModuleItem) -> Self {
    Self { node }
  }
}

#[derive(Debug)]
pub struct ExportStatementNonDecl {
  pub node: ModuleItem,
}

impl ExportStatementNonDecl {
  pub fn new(node: ModuleItem) -> Self {
    Self { node }
  }
}

#[derive(Debug)]
pub struct DeclStatement {
  pub node: ModuleItem,
  pub included: bool,
  pub reads: HashSet<Mark>,
  // This includes export named declarations / export default declarations / export namespaced declarations,
  // since these should be transformed
  pub is_export_decl: bool,

  // `tree-shaking` is supported by including this mark
  // `mark` equals to the mark of node's declaration's ident
  pub mark: Mark,
}

impl DeclStatement {
  pub fn new(node: ModuleItem) -> Self {
    Self {
      node,
      included: false,
      is_export_decl: Default::default(),
      reads: Default::default(),
      mark: Default::default(),
    }
  }

  pub fn validate_node_type(&self) {
    log::debug!(
      "[DeclStatement] validating node {:?} with is_export_decl set to {}...",
      self.node,
      self.is_export_decl
    );

    if matches!(
      self.node,
      ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(_))
        | ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(_))
    ) {
      assert!(
        self.is_export_decl,
        "[Statement]: failed to validate node type of {:#?} with `is_export_decl` {}",
        self.node, self.is_export_decl
      )
    }

    if let ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(export_named)) = &self.node {
      assert_eq!(
        matches!(
          export_named.specifiers.get(0),
          Some(ExportSpecifier::Namespace(_))
        ),
        self.is_export_decl,
        "[Statement]: failed to validate node type of {:#?} with `is_export_decl` {}",
        self.node,
        self.is_export_decl
      )
    }
  }
}
