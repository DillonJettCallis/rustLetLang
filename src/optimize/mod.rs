use bytecode::{BitModule, BitFunction};
use optimize::load_store_optimizer::load_store_opt;

mod load_store_optimizer;

pub struct Optimizer {
  ops: Vec<Box<Fn(&mut BitModule, &mut BitFunction) -> ()>>
}

impl Optimizer {

  pub fn new() -> Optimizer {
    // Hold this one back until we figure out how to clean up jumps Box::new(load_store_opt)

    Optimizer {
      ops: vec![]
    }
  }

  pub fn optimize(&self, module: &mut BitModule, func: &mut BitFunction) {
    self.ops.iter().for_each(|op| op(module, func));
  }

  pub fn register(&mut self, func: Box<Fn(&mut BitModule, &mut BitFunction) -> ()>) {
    self.ops.push(func)
  }

}

