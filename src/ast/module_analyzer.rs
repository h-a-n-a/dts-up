use swc_ecma_visit::VisitMut;

enum VariableDeclaration {
  TsInterfaceDeclaration,
  TsTypeAliasDeclaration,
  TsEnumDeclaration,
  VariableDeclaration,
  FunctionDeclaration,
  ClassDeclaration,
}

struct Scope {
  // definitions:
}

struct ModuleAnalyzer {}

impl VisitMut for ModuleAnalyzer {}
