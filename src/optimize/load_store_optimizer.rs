use ir::{IrFunction, Ir};

pub fn load_store_opt(func: &mut IrFunction) {
  load_store(&mut func.body);
}

fn load_store(body: &mut Vec<Ir>) {
  let mut index = 0usize;
  let mut do_remove = false;

  while index < body.len() - 2 {
    if let Ir::StoreValue {local: ref store} = body[index] {
      if let Ir::LoadValue{local: ref load} = body[index + 1] {
        if store == load {
          if let Ir::FreeLocal {local: ref free} = body[index + 2] {
            if load == free {
              do_remove = true
            }
          }
        }
      }
    }

    if let Ir::Branch {ref mut then_block, ref mut else_block} = body[index] {
      load_store( then_block);
      load_store( else_block);
    }

    if do_remove {
      body.drain(index..index + 2);
      do_remove = false;
    } else {
      index += 1;
    }
  }
}

