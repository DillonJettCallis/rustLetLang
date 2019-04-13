use optimize::Optimizer;
use bytecode::{BitModule, BitFunction, Instruction};

// TODO: Fill in jumps
pub fn load_store_opt(module: &mut BitModule, func: &mut BitFunction) {
  let mut index = 0usize;
  let body = &mut func.body;

  while index < body.len() - 1 {
    match body[index] {
      Instruction::StoreValue {local: store} => {
        if let Instruction::LoadValue{local: load} = body[index + 1] {
          if store == load {
            let mut inner = index + 2;
            let mut found_reset = false;

            while !found_reset && inner < body.len() {
              match body[inner] {
                Instruction::StoreValue{local: next_store} if next_store == store => break,
                Instruction::LoadValue{local: next_load} if next_load == store => found_reset = true,
                _ => {}
              }

              inner += 1;
            }

            if found_reset {
              // Just skip the following load
              index += 1;
            } else {
              body.drain(index..index + 2);
              // removing two elements means we want to go back one.
              index -= 1;
            }
          }
        }
      }
      _ => {}
    }

    index += 1;
  }

}

