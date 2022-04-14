use std::collections::{HashMap, HashSet};

use swc_atoms::JsWord;
use swc_common::Mark;
use swc_ecma_visit::VisitMut;

enum VariableDeclaration {
  TsInterfaceDeclaration,
  TsTypeAliasDeclaration,
  TsEnumDeclaration,
  VariableDeclaration,
  FunctionDeclaration,
  ClassDeclaration,
}

struct Definition {
  definition_type: VariableDeclaration,
  mark: Mark,
}

struct Scope {
  definitions: HashMap<JsWord, Definition>,
  reads: HashSet<JsWord>,
}

impl Scope {
  fn define(&mut self, name: JsWord, definition_type: VariableDeclaration) {
    use std::collections::hash_map::Entry;
    match self.definitions.entry(name) {
      Entry::Vacant(vacant) => {
        let def = Definition {
          definition_type,
          mark: Mark::fresh(Mark::default()),
        };
        vacant.insert(def);
      }
      Entry::Occupied(occupied) => {
        // Interfaces may be defined multiple times in the same scope
      }
    }
  }

  fn read(&mut self, name: JsWord) {
    self.reads.insert(name);
  }
}

pub struct ModuleAnalyzer {
  scope: Vec<Scope>,
}

impl ModuleAnalyzer {
  fn new() -> Self {
    Self {
      scope: Default::default(),
    }
  }
}

impl VisitMut for ModuleAnalyzer {}
