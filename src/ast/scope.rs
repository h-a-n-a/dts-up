use std::collections::{HashMap, HashSet};

use swc_atoms::JsWord;
use swc_common::Mark;

use super::symbol;

#[derive(Debug)]
pub enum VariableDeclaration {
  TsInterfaceDeclaration,
  TsTypeAliasDeclaration,
  TsTypeParameter,
  TsEnumDeclaration,
  VariableDeclaration,
  FunctionDeclaration,
  ClassDeclaration,
}

#[derive(Debug)]
pub struct Definition {
  pub definition_type: VariableDeclaration,
  pub mark: Mark,
}

/// Good to notice:
/// Type and value declarations are actually two totally different stuff, but in this scenario, single scope makes sense.
#[derive(Debug)]
pub struct Scope {
  pub definitions: HashMap<JsWord, Definition>,
  pub mark_to_name: HashMap<Mark, JsWord>,
  pub reads: HashSet<Mark>,
}

impl Scope {
  fn get_variable_definition(&self, name: JsWord) -> Option<&Definition> {
    self.definitions.get(&name)
  }

  fn add_variable_definition(&mut self, name: JsWord, definition_type: VariableDeclaration) {
    use std::collections::hash_map::Entry;

    match self.definitions.entry(name) {
      Entry::Vacant(vacant) => {
        // let def = Definition { definition_type };
        // vacant.insert(def);
      }
      Entry::Occupied(occupied) => {
        // Interfaces may be defined multiple times in the same scope
      }
    }
  }

  fn add_variable_read(&mut self, variable_mark: Mark) {
    self.reads.insert(variable_mark);
  }
}
