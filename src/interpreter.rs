use std::collections::HashMap;
use std::fmt::Debug;
use std::fmt::Error;
use std::fmt::Formatter;
use std::rc::Rc;

use bytebuffer::ByteBuffer;
use simple_error::SimpleError;

use bytecode::*;
use runtime::Value;
use shapes::*;
use shapes::Shape::SimpleFunctionShape;

pub trait RunFunction {

  fn execute(&self, machine: &Machine, args: Vec<Value>) -> Result<Value, SimpleError>;

  fn get_shape(&self) -> &Shape;

}

impl Debug for RunFunction {
  fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
    f.write_str("<function>")
  }
}

pub struct Machine {
  app: AppDirectory,
  core_functions: HashMap<String, Rc<RunFunction>>,
}

impl Machine {

  pub fn new(app: AppDirectory) -> Machine {
    let mut core_functions: HashMap<String, Rc<RunFunction>> = HashMap::new();
    core_functions.insert(String::from("Core.+"), Rc::new(float_op("Core.+", |l, r| l + r)));
    core_functions.insert(String::from("Core.-"), Rc::new(float_op("Core.-", |l, r| l - r)));
    core_functions.insert(String::from("Core.*"), Rc::new(float_op("Core.*", |l, r| l * r)));
    core_functions.insert(String::from("Core./"), Rc::new(float_op("Core./", |l, r| l / r)));
    Machine{app, core_functions}
  }

  pub fn run_main(&self) -> Result<Value, SimpleError> {
    let main = self.app.functions.get("main")
      .ok_or_else(|| SimpleError::new("No main function"))?;

    main.execute(self, vec![])
  }

  pub fn execute(&self, func: &BitFunction, mut locals: Vec<Value>) -> Result<Value, SimpleError> {
    let mut index = 0usize;
    let mut stack: Vec<Value> = Vec::new();
    locals.resize(func.max_locals as usize, Value::Null);

    while index < func.body.len() {
      match func.body[index] {
        Instruction::NoOp => {},
        Instruction::Duplicate => {
          let last = stack.last()
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to duplicate empty stack"))?
            .clone();
          stack.push(last);
        },
        Instruction::Pop => {
          stack.pop()
            .ok_or_else(|| SimpleError::new("Invalid bytecode in module. Attempt to pop empty stack"))?;
        },
        Instruction::Swap => {
          let first = stack.pop()
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to swap empty stack"))?;

          let second = stack.pop()
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to swap stack of 1"))?;

          stack.push(first );
          stack.push(second);
        },
        Instruction::LoadConstNull => {
          stack.push(Value::Null);
        },
        Instruction::LoadConst{ref kind, const_id} => {
          match kind {
            LoadType::String => {
              let index = const_id as usize;

              let value: &String = self.app.string_constants.get(index)
                .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid String constant id"))?;

              stack.push(Value::String(Rc::new(value.clone())));
            },
            LoadType::Function => {
              let index = const_id as usize;

              let func_ref = &self.app.function_refs[index];
              let boxed = self.app.functions.get(&func_ref.name)
                .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid Function constant id"))?
                .clone();

              stack.push(Value::Function(boxed));
            }
            _ => return Err(SimpleError::new("Invalid bytecode. LoadConst of invalid kind"))
          }
        },
        Instruction::LoadConstFloat{value} => stack.push(Value::Float(value)),
        Instruction::LoadValue{local} => {
          let index = local as usize;

          let local: &Value = locals.get(index)
            .ok_or_else(|| SimpleError::new("Invalid bytecode. LoadValue of local that doesn't exist"))?;

          stack.push(local.clone());
        },
        Instruction::StoreValue{local} => {
          let index = local as usize;

          let value = stack.pop()
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to StoreValue of empty stack"))?;

          locals[index] = value;
        },
        Instruction::CallStatic{func_id} => {
          let func_ref: &FunctionRef = self.app.function_refs.get(func_id as usize)
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid function id"))?;

          let func = self.core_functions.get(&func_ref.name)
            .or_else(||self.app.functions.get(&func_ref.name))
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Function with name not found"))?;

          let shape = func.get_shape();

          if let Shape::SimpleFunctionShape { args, result: _ } = shape {
            let size = args.len();
            let mut params: Vec<Value> = Vec::with_capacity(size);

            for i in 0..size {
              let param = stack.pop()
                .ok_or_else(|| SimpleError::new("Invalid bytecode. Not enough args for function"))?;

              params.push(param);
            }

            params.reverse();

            let result = func.execute(&self, params)?;
            stack.push(result);
          } else {
            return Err(SimpleError::new("Invalid bytecode. CallStatic is not function"))
          }
        },
        Instruction::CallDynamic{shape_id} => {
          let shape: &Shape = self.app.shape_refs.get(shape_id as usize)
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid Shape constant"))?;

          if let Shape::SimpleFunctionShape { args, result: _ } = shape {
            let size = args.len();
            let mut params: Vec<Value> = Vec::with_capacity(size);

            for i in 0..size {
              let param = stack.pop()
                .ok_or_else(|| SimpleError::new("Invalid bytecode. Not enough args for function"))?;

              params.push(param);
            }

            params.reverse();

            let maybe_func: Value = stack.pop()
              .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid built in function id"))?;

            if let Value::Function(func) = maybe_func {
              if func.get_shape() == shape {
                let result = func.execute(&self, params)?;
                stack.push(result);
              } else {
                return Err(SimpleError::new("Invalid bytecode. CallDynamic function shape different than provided shape"))
              }
            } else {
              return Err(SimpleError::new("Invalid bytecode. CallDynamic is not function"))
            }
          } else {
            return Err(SimpleError::new("Invalid bytecode. CallDynamic is not function"))
          }
        },
        Instruction::Return => {
          return stack.pop()
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to return empty stack"));
        },
        Instruction::IfEqual{jump} => {
          let first = stack.pop()
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to IfEqual empty stack"))?;

          let second = stack.pop()
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to IfEqual stack of 1 item"))?;

          match (first, second) {
            (Value::Float(first_value), Value::Float(second_value)) => {
              if first_value == second_value {
                index = Machine::calculate_jump(index, jump);
              }
            }
            _ => {
              // Do nothing
            }
          }
        },
        Instruction::Jump{jump} => {
          index = Machine::calculate_jump(index, jump);
        }
        Instruction::Debug => {
          println!("Debug: \n  Stack: {:#?}\n  Locals: {:#?}", &stack, &locals)
        }

        _ => unimplemented!()
      }

      index += 1;
    }

    Err(SimpleError::new(format!("Overflowed function body")))
  }

  fn calculate_jump(index: usize, jump: i32) -> usize {
    if jump >= 0 {
      return index + (jump as usize) - 1;
    } else {
      let rel = (0 - jump) as usize;
      return index - rel - 1;
    }
  }
}

impl RunFunction for BitFunction {

  fn execute(&self, machine: &Machine, args: Vec<Value>) -> Result<Value, SimpleError> {
    machine.execute(self, args)
  }

  fn get_shape(&self) -> &Shape {
    &self.shape
  }
}

struct NativeFunction<T> {
  func: T,
  shape: Shape,
}

impl<T: Fn(&Machine, Vec<Value>) -> Result<Value, SimpleError>> RunFunction for NativeFunction<T> {
  fn execute(&self, machine: &Machine, args: Vec<Value>) -> Result<Value, SimpleError> {
    (self.func)(machine, args)
  }

  fn get_shape(&self) -> &Shape {
    &self.shape
  }
}

fn float_op<Op: Fn(f64, f64) -> f64>(name: &'static str, op: Op) -> impl RunFunction {
  let func = move |machine: &Machine, args: Vec<Value>| {
    if args.len() == 2 {
      if let Value::Float(first) = args[0] {
        if let Value::Float(second) = args[1] {
          let result = op(first, second);
          return Ok(Value::Float(result));
        }
      }
    }

    return Err(SimpleError::new(format!("{} takes exactly two float arguments", name)));
  };

  NativeFunction {
    func,
    shape: Shape::SimpleFunctionShape {
      args: vec![shape_float(), shape_float()],
      result: Box::new(shape_float()),
    },
  }
}
