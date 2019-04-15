use bytecode::{BitModule, BitFunction};
use optimize::load_store_optimizer::load_store_opt;
use ir::IrFunction;
use optimize::free_local_optimizer::free_local_opt;
use optimize::lift_return_optimizer::lift_return_opt;

mod load_store_optimizer;
mod free_local_optimizer;
mod lift_return_optimizer;

pub struct Optimizer {
  ops: Vec<Box<Fn(&mut IrFunction) -> ()>>
}

impl Optimizer {

  pub fn new() -> Optimizer {
    Optimizer {
      ops: vec![
        Box::new(lift_return_opt),
        Box::new(free_local_opt),
        Box::new(load_store_opt),
      ]
    }
  }

  pub fn optimize(&self, func: &mut IrFunction) {
    self.ops.iter().for_each(|op| op(func));
  }

  pub fn register(&mut self, func: Box<Fn(&mut IrFunction) -> ()>) {
    self.ops.push(func)
  }

}

