use ir::{IrFunction, Ir};

pub fn lift_return_opt(func: &mut IrFunction) {
  lift_return(&mut func.body);
}

fn lift_return(body: &mut Vec<Ir>) {
  let mut index = body.len() - 1;
  let mut do_remove = false;

  while index > 0 {
    match body[index] {
      Ir::Return => {
        if let Ir::Branch {ref mut then_block, ref mut else_block} = body[index - 1] {
          then_block.push(Ir::Return);
          lift_return(then_block);
          else_block.push(Ir::Return);
          lift_return(else_block);
          do_remove = true;
        }
      }
      _ => {}
    }

    if do_remove {
      body.remove(index);
      do_remove = false;
    }

    index -= 1;
  }
}
