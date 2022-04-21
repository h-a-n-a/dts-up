use std::collections::{HashMap, HashSet};
use std::fmt::Formatter;

use petgraph::graph::{Edges, Graph, NodeIndex};
use petgraph::{adj::DefaultIx, visit::EdgeRef, Directed, Direction};
use smol_str::SmolStr;

use crate::ast::module::ModuleId;

#[derive(Debug, Clone)]
pub struct ModuleEdgeImport {
  pub index: u32,
}

#[derive(Debug, Clone)]
pub struct ModuleEdgeExportAll {
  pub index: u32,
}

#[derive(Debug, Clone)]
pub struct ModuleEdgeExportNamed {
  pub index: u32,
}

#[derive(Debug, Clone)]
pub struct ModuleEdgeExportNamespace {
  pub index: u32,
}

#[derive(Debug, Clone)]
pub enum ModuleEdge {
  Import(ModuleEdgeImport),
  // currently not supported
  // DynamicImport,
  ExportAll(ModuleEdgeExportAll),
  ExportNamed(ModuleEdgeExportNamed),
  ExportNamespace(ModuleEdgeExportNamespace),
}

pub type ModuleIndex = NodeIndex;

pub struct ModuleGraph {
  inner: Graph<ModuleId, ModuleEdge, Directed, DefaultIx>,
  sorted_modules: Vec<ModuleIndex>,
  module_id_to_index: HashMap<ModuleId, ModuleIndex>,
}

impl std::fmt::Debug for ModuleGraph {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:#?}", self.inner)
  }
}

impl ModuleGraph {
  pub fn new() -> Self {
    Self {
      inner: Default::default(),
      sorted_modules: Default::default(),
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

  pub fn get_module_id_by_index(&self, module_index: &ModuleIndex) -> ModuleId {
    self.inner[*module_index].clone()
  }

  pub fn get_module_index_by_id(&self, module_id: &ModuleId) -> Option<ModuleIndex> {
    self
      .module_id_to_index
      .get(module_id)
      .map(|index| index.clone())
  }

  pub fn get_edges_directed(
    &self,
    module_index: ModuleIndex,
    direction: Direction,
  ) -> Edges<ModuleEdge, Directed> {
    self.inner.edges_directed(module_index, direction)
  }

  pub fn sort_modules(&mut self, entry_module_index: ModuleIndex) {
    let mut sorted: Vec<ModuleIndex> = Default::default();
    let mut stack = vec![entry_module_index];
    let mut visited: HashSet<ModuleIndex> = Default::default();

    while let Some(node_index) = stack.pop() {
      if visited.contains(&node_index) {
        sorted.push(node_index);
        continue;
      }

      stack.push(node_index);
      visited.insert(node_index);

      let mut level_edges = self
        .get_edges_directed(node_index, Direction::Outgoing)
        .filter_map(|edge| {
          let target_module_index = edge.target();
          let weight = edge.weight();

          if visited.contains(&target_module_index) {
            return None;
          }

          match weight {
            ModuleEdge::Import(module_import) => Some((target_module_index, module_import.index)),
            ModuleEdge::ExportAll(module_export) => {
              Some((target_module_index, module_export.index))
            }
            ModuleEdge::ExportNamed(module_export) => {
              Some((target_module_index, module_export.index))
            }
            ModuleEdge::ExportNamespace(module_export) => {
              Some((target_module_index, module_export.index))
            }
          }
        })
        .collect::<Vec<_>>();

      level_edges.sort_by_key(|e| e.1);

      level_edges
        .iter()
        .for_each(|(module_index, _)| stack.push(module_index.clone()))
    }

    self.sorted_modules = sorted;
  }

  #[inline]
  pub fn get_sorted_modules(&self) -> &Vec<ModuleIndex> {
    &self.sorted_modules
  }
}
