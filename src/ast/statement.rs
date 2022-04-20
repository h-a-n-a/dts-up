use std::collections::HashSet;

use swc_common::Mark;
use swc_ecma_ast::ModuleItem;

#[derive(Debug)]
pub enum Statement {
  DeclStatement(DeclStatement),
  ImportStatement(ImportStatement),
  ExportStatement(ExportStatement),
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
pub struct ExportStatement {
  pub node: ModuleItem,
}

impl ExportStatement {
  pub fn new(node: ModuleItem) -> Self {
    Self { node }
  }
}

#[derive(Debug)]
pub struct DeclStatement {
  pub node: ModuleItem,
  pub included: bool,
  pub reads: HashSet<Mark>,

  // `tree-shaking` is supported by including this mark
  // `mark` equals to the mark of node's declaration's ident
  pub mark: Mark,
}

impl DeclStatement {
  pub fn new(node: ModuleItem) -> Self {
    Self {
      node,
      included: false,
      reads: Default::default(),
      mark: Default::default(),
    }
  }
}
