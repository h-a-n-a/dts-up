use std::collections::{HashMap, HashSet};

use smol_str::SmolStr;

use crate::ast::parse::SOURCE_MAP;
use crate::ast::{
  module::{Exports, LocalName},
  statement::{DeclStatement, Statement},
};
use crate::finalizer::Finalizer;
use crate::graph::{Graph, GraphOptions, ModuleIndex};
use crate::result::{DtsupErrorType, Error};

type TopLevelExports = HashMap<LocalName, Exports>;

pub struct Dtsup {
  entry: SmolStr,
  graph: Option<Box<Graph>>,
}

pub struct DtsupOptions<T: AsRef<str>> {
  pub entry: T,
}

pub struct DtsupGenerateOptions<T: AsRef<str>> {
  pub outdir: T,
}

impl Dtsup {
  pub fn new<T>(options: DtsupOptions<T>) -> Self
  where
    T: AsRef<str>,
  {
    Self {
      entry: options.entry.into(),
      graph: Default::default(),
    }
  }

  fn generate_build_result(&self, graph: &Box<Graph>) -> (Vec<DeclStatement>, TopLevelExports) {
    let mut visited: HashSet<ModuleIndex> = Default::default();

    let decl_statement = graph
      .get_sorted_modules()
      .clone()
      .into_iter()
      .flat_map(|module_index| {
        if visited.contains(&module_index) {
          return Default::default();
        }
        visited.insert(module_index);

        let module = self
          .graph
          .as_ref()
          .unwrap()
          .get_module_by_module_index(&module_index);

        module
          .statements
          .iter()
          .filter_map(|s| match s {
            Statement::DeclStatement(s) => {
              if s.included {
                return Some(s.clone());
              }
              None
            }
            _ => None,
          })
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>();

    (decl_statement, graph.get_top_level_exports())
  }

  pub async fn build(&mut self) -> Result<(), Error> {
    let mut graph = Box::new(Graph::new(GraphOptions {
      entry: self.entry.clone(),
    }));

    graph.build().await?;

    self.graph = Some(graph);

    Ok(())
  }

  fn generate_with_graph<T>(
    &self,
    graph: &Box<Graph>,
    options: DtsupGenerateOptions<T>,
  ) -> Result<(), Error>
  where
    T: AsRef<str>,
  {
    use swc_ecma_ast::{EsVersion, ModuleItem};
    use swc_ecma_codegen::text_writer::JsWriter;
    use swc_ecma_visit::FoldWith;

    let (mut decl_statement, top_level_exports) = self.generate_build_result(graph);

    let mut finalizer = Finalizer { top_level_exports };

    let mut folded_statements = decl_statement
      .into_iter()
      .map(|s| s.node.fold_with(&mut finalizer))
      .collect::<Vec<ModuleItem>>();

    let top_level_export_statement = finalizer.generate_exports();
    folded_statements.push(top_level_export_statement);

    let mut output = Default::default();

    let mut emitter = swc_ecma_codegen::Emitter {
      cfg: Default::default(),
      cm: SOURCE_MAP.clone(),
      comments: None,
      wr: Box::new(JsWriter::with_target(
        SOURCE_MAP.clone(),
        "\n",
        &mut output,
        None,
        EsVersion::latest(),
      )),
    };

    folded_statements.iter().for_each(|s| {
      emitter.emit_module_item(s).unwrap();
    });

    let code = String::from_utf8(output)?;
    log::debug!("[Dtsup] code generated: \n{}", code);

    Ok(())
  }

  pub fn generate<T>(&self, options: DtsupGenerateOptions<T>) -> Result<(), Error>
  where
    T: AsRef<str>,
  {
    if let Some(graph) = &self.graph {
      self.generate_with_graph(graph, options);
      Ok(())
    } else {
      Err(Error::new_with_reason(
        DtsupErrorType::GraphMissingError,
        "[Bundle] unable to find graph, run `build` at first to resolve this issue.",
      ))
    }
  }
}
