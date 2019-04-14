use ir::{IrFunction, Ir};

pub fn load_store_opt(func: &mut IrFunction) {
  let mut index = 0usize;
  let body = &mut func.body;
  let mut do_remove = false;

  while index < body.len() - 1 {
    match body[index] {
      Ir::StoreValue {local: ref store} => {
        if let Ir::LoadValue{local: ref load} = body[index + 1] {
          if store == load {
            let mut inner = index + 2;
            let mut found_reset = false;

            while !found_reset && inner < body.len() {
              match body[inner] {
                Ir::StoreValue{local: ref next_store} if next_store == store => break,
                Ir::LoadValue{local: ref next_load} if next_load == store => found_reset = true,
                _ => {}
              }

              inner += 1;
            }

            if found_reset {
              // Just skip the following load
              index += 1;
            } else {
              do_remove = true
            }
          }
        }
      }
      _ => {}
    }

    if do_remove {
      body.drain(index..index + 2);
      do_remove = false;
    } else {
      index += 1;
    }
  }

}

