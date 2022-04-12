#[tokio::main]
async fn main() {
  env_logger::init();
  use dts_up::graph::{ModuleGraph, ModuleGraphOptions};

  let mut graph = ModuleGraph::new(ModuleGraphOptions {
    entry: vec!["node-tests/import-default-interface/index.d.ts"],
  });

  graph.generate().await.unwrap();

  println!("{:?}", graph);
}
