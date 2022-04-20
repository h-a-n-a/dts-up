use std::collections::HashMap;

use petgraph::graph::{Edges, Graph, NodeIndex};
use petgraph::{adj::DefaultIx, Directed, Direction};
use smol_str::SmolStr;

use crate::ast::module::ModuleId;

#[derive(Debug)]
pub struct ModuleEdgeImport {
  pub index: u32,
}

#[derive(Debug)]
pub struct ModuleEdgeReExport {}

#[derive(Debug)]
pub enum ModuleEdge {
  Import(ModuleEdgeImport),
  // currently not supported
  // DynamicImport,
  Export,
}

pub type ModuleIndex = NodeIndex;

#[derive(Debug)]
pub struct ModuleGraph {
  pub inner: Graph<ModuleId, ModuleEdge, Directed, DefaultIx>,
  module_id_to_index: HashMap<ModuleId, ModuleIndex>,
}

impl ModuleGraph {
  pub fn new() -> Self {
    Self {
      inner: Default::default(),
      module_id_to_index: Default::default(),
    }
  }

  pub fn add_module(&mut self, module_id: SmolStr) -> ModuleIndex {
    let module_index = self.inner.add_node(module_id.clone());

    self
      .module_id_to_index
      .entry(module_id)
      .or_insert(module_index);

    module_index
  }

  pub fn get_or_add_module(&mut self, module_id: SmolStr) -> ModuleIndex {
    if let Some(module_index) = self.module_id_to_index.get(&module_id) {
      module_index.clone()
    } else {
      self.add_module(module_id)
    }
  }

  pub fn add_edge(
    &mut self,
    from_module_idx: ModuleIndex,
    to_module_idx: ModuleIndex,
    edge: ModuleEdge,
  ) {
    self.inner.add_edge(from_module_idx, to_module_idx, edge);
  }

  pub fn get_module_id_by_index(&self, module_index: ModuleIndex) -> ModuleId {
    self.inner[module_index].clone()
  }

  pub fn get_module_index_by_id(&self, module_id: ModuleId) -> Option<ModuleIndex> {
    self
      .module_id_to_index
      .get(&module_id)
      .map(|index| index.clone())
  }

  pub fn get_edges_directed(
    &self,
    module_index: ModuleIndex,
    direction: Direction,
  ) -> Edges<ModuleEdge, Directed> {
    self.inner.edges_directed(module_index, direction)
  }
}
