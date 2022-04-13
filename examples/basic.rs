#[tokio::main]
async fn main() {
  env_logger::init();
  use dts_up::graph::{Graph, GraphOptions};

  let mut graph = Graph::new(GraphOptions {
    entry: vec!["node-tests/import-default-interface/index.d.ts"],
  });

  graph.generate().await.unwrap();

  println!("\n\n Graph generated: {:?}", graph);
}
