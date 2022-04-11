use petgraph::Graph;
use smol_str::SmolStr;

pub struct ModuleGraph {
  entry: Vec<SmolStr>,
}

pub struct ModuleGraphOptions<T: AsRef<str>> {
  entry: Vec<T>,
}

struct Msg {}

impl ModuleGraph {
  pub fn new<T>(options: ModuleGraphOptions<T>) -> Self {
    let entry = options
      .entry
      .iter()
      .map(|item| SmolStr::new(item))
      .collect::<Vec<SmolStr>>();

    Self { entry }
  }

  pub fn generate(&mut self) {
    let entry = &self.entry[0];
    let num_of_threads = num_cpus::get();

    let (tx, rx) = crossbeam_channel::unbounded::<Msg>();

    for _ in 0..num_of_threads {
      std::thread::spawn(move || loop {})
    }
  }
}
