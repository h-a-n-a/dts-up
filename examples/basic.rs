#[tokio::main]
async fn main() {
  env_logger::init();
  // use dts_up::graph::{Graph, GraphOptions};
  use dts_up::{Dtsup, DtsupGenerateOptions, DtsupOptions};

  let mut bundler = Dtsup::new(DtsupOptions {
    entry: "node-tests/_test/index.d.ts",
  });

  bundler.build().await.unwrap();
  bundler.generate(DtsupGenerateOptions { outdir: "abc" });

  // println!("\n\n Graph generated: {:?}", graph);
}
