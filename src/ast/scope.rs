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
pub enum ScopeKind {
  TsTypeParameter,
  TypeScope,
}

#[derive(Debug)]
pub struct Definition {
  pub kind: VariableDeclaration,
  pub mark: Mark,
}

impl Definition {
  pub fn new(mark: Mark, kind: VariableDeclaration) -> Self {
    Self { mark, kind }
  }
}

/// Good to notice:
/// Type and value declarations are actually two totally different stuff, but in this scenario, single scope type makes sense.
/// In JavaScript, we should differentiate scopes into function scope, block scope, etc.
#[derive(Debug)]
pub struct Scope {
  pub(crate) kind: ScopeKind,
  pub(crate) definitions: HashMap<JsWord, Definition>,
  mark_to_name: HashMap<Mark, JsWord>,
  reads: HashSet<Mark>,
}

impl Scope {
  pub fn new(kind: ScopeKind) -> Self {
    Self {
      kind,
      definitions: Default::default(),
      mark_to_name: Default::default(),
      reads: Default::default(),
    }
  }

  pub fn get_scope_kind(&self) -> &ScopeKind {
    &self.kind
  }

  pub fn get_variable_definition(&self, name: &JsWord) -> Option<&Definition> {
    self.definitions.get(name)
  }

  pub fn add_variable_definition(
    &mut self,
    name: JsWord,
    definition_type: VariableDeclaration,
    mark: Mark,
  ) {
    use std::collections::hash_map::Entry;

    match self.definitions.entry(name.clone()) {
      Entry::Vacant(vacant) => {
        vacant.insert(Definition::new(mark.clone(), definition_type));
        self.mark_to_name.insert(mark, name);
      }
      Entry::Occupied(occupied) => {
        // Interfaces may be defined multiple times in the same scope
        if !matches!(definition_type, VariableDeclaration::TsInterfaceDeclaration) {
          panic!(
            "[Scope] unable to declare {:?} multiple times",
            definition_type
          )
        }
      }
    }
  }

  pub fn add_variable_read(&mut self, variable_mark: Mark) {
    self.reads.insert(variable_mark);
  }
}
