use ir::{IrFunction, Ir};

/**
* Finds the pattern of
* Store(x)
* Load(x)
*
* If this is followed by a Free(x)
*   Remove all three. The variable is never used again, don't bother storing it.
* else
*   Remove the Load(x) and insert a Duplicate before store. Duplicate should be cheaper than Load.
*/
pub fn load_store_opt(func: &mut IrFunction) {
  load_store(&mut func.body);
}

fn load_store(body: &mut Vec<Ir>) {
  let mut index = 0usize;
  let mut do_remove = false;
  let mut do_dup = false;

  while index < body.len() - 2 {
    if let Ir::StoreValue {local: ref store} = body[index] {
      if let Ir::LoadValue{local: ref load} = body[index + 1] {
        if store == load {
          if let Ir::FreeLocal {local: ref free} = body[index + 2] {
            if load == free {
              do_remove = true;
            }
          } else {
            do_dup = true;
          }
        }
      }
    }

    if let Ir::Branch {ref mut then_block, ref mut else_block} = body[index] {
      load_store( then_block);
      load_store( else_block);
    }

    if do_remove {
      body.drain(index..index + 3);
      do_remove = false;
    } else if do_dup {
      body.remove(index + 1);
      body.insert(index, Ir::Duplicate);
      index += 1;
      do_dup = false;
    } else {
      index += 1;
    }
  }
}

