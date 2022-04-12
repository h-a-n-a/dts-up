use dashmap::{DashMap, DashSet};
use swc_common::Mark;
use swc_ecma_ast::ModuleItem;

#[derive(Debug)]
pub struct Statement {
  pub node: ModuleItem,
  pub included: bool,
  pub reads: DashSet<Mark>,
  pub writes: DashSet<Mark>,
}

impl Statement {
  pub fn new(node: ModuleItem) -> Self {
    Self {
      node,
      included: false,
      reads: Default::default(),
      writes: Default::default(),
    }
  }
}
