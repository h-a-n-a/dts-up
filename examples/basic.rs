#[tokio::main]
async fn main() {
  env_logger::init();
  use dts_up::graph::{Graph, GraphOptions};

  let mut graph = Graph::new(GraphOptions {
    entry: vec!["node-tests/_test/index.d.ts"],
  });

  graph.generate().await.unwrap();

  println!("\n\n Graph generated: {:?}", graph);
}
