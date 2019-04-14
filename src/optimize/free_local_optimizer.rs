use ir::{IrFunction, Ir};

pub fn free_local_opt(func: &mut IrFunction) {
  free_local(&mut func.body, &Vec::new());
}

fn free_local(body: &mut Vec<Ir>, prev_locals: &Vec<String>) {
  let mut index = body.len();
  let mut known_locals = prev_locals.clone();
  let mut do_free = false;

  while index > 0 {
    match body[index - 1] {
      Ir::LoadValue{local: ref next_load} => {
        if !known_locals.contains(next_load) {
          known_locals.push(next_load.clone());
          do_free = true;

        }
      }
      Ir::Branch {ref mut then_block, ref mut else_block} => {

        free_local( then_block, &known_locals);
        free_local( else_block, &known_locals);
      }
      _ => {}
    }

    if do_free {
      body.insert(index, Ir::FreeLocal {local: known_locals.last().unwrap().clone()});
      do_free = false;
    }

    index -= 1;
  }

}
