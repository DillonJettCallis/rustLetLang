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
  app: BitApplication,
  core_functions: HashMap<String, Rc<RunFunction>>,
}

impl Machine {

  pub fn new(app: BitApplication) -> Machine {
    let mut core_functions: HashMap<String, Rc<RunFunction>> = HashMap::new();
    core_functions.insert(String::from("+"), Rc::new(float_op("+", |l, r| l + r)));
    core_functions.insert(String::from("-"), Rc::new(float_op("-", |l, r| l - r)));
    core_functions.insert(String::from("*"), Rc::new(float_op("*", |l, r| l * r)));
    core_functions.insert(String::from("/"), Rc::new(float_op("/", |l, r| l / r)));
    Machine{app, core_functions}
  }

  pub fn run_main(&self) -> Result<Value, SimpleError> {
    let (ref main_package, ref main_module) = self.app.main;

    let main = self.app.packages.get(main_package)
      .and_then(|package| package.modules.get(main_module))
      .and_then(|module| module.functions.get("main"))
      .ok_or_else(|| SimpleError::new("No main function"))?;

    main.execute(self, vec![])
  }

  pub fn execute(&self, func: &BitFunction, mut locals: Vec<Value>) -> Result<Value, SimpleError> {
    let module = self.app.packages.get(&func.package)
      .and_then(|package| package.modules.get(&func.module))
      .ok_or_else(|| SimpleError::new("BitFunction lookup failed"))?;

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

              let value: &String = module.string_constants.get(index)
                .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid String constant id"))?;

              stack.push(Value::String(Rc::new(value.clone())));
            },
            LoadType::Function => {
              let index = const_id as usize;

              let func_ref = &module.function_refs[index];
              let boxed = module.functions.get(&func_ref.name)
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
          let func_ref: &FunctionRef = module.function_refs.get(func_id as usize)
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid function id"))?;

          let func = self.core_functions.get(&func_ref.name)
            .or_else(||module.functions.get(&func_ref.name))
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
        Instruction::CallDynamic{param_count} => {
          let mut params: Vec<Value> = Vec::with_capacity(param_count as usize);

          for i in 0..param_count {
            let param = stack.pop()
              .ok_or_else(|| SimpleError::new("Invalid bytecode. Not enough args for function"))?;

            params.push(param);
          }

          params.reverse();

          let maybe_func: Value = stack.pop()
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid built in function id"))?;

          if let Value::Function(func) = maybe_func {
            let result = func.execute(&self, params)?;
            stack.push(result);
          } else {
            return Err(SimpleError::new("Invalid bytecode. CallDynamic is not function"))
          }
        },
        Instruction::BuildClosure {param_count, func_id} => {
          let func_ref: &FunctionRef = module.function_refs.get(func_id as usize)
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid function id"))?;

          let func = self.core_functions.get(&func_ref.name)
            .or_else(||module.functions.get(&func_ref.name))
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Function with name not found"))?
            .clone();

          let mut params = Vec::with_capacity(param_count as usize);

          for _ in 0..param_count {
            let param = stack.pop()
              .ok_or_else(|| SimpleError::new("Invalid bytecode. Not enough args for closure"))?;
            params.push(param);
          }

          params.reverse();

          let closure = ClosureFunction {
            func,
            closures: params
          };

          stack.push(Value::Function(Rc::new(closure)));
        },
        Instruction::Return => {
          return stack.pop()
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to return empty stack"));
        },
        Instruction::Branch{jump} => {
          let first = stack.pop()
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to Branch empty stack"))?;

          match first {
            Value::True => {},
            Value::False => index = Machine::calculate_jump(index, jump),
            _ => return Err(SimpleError::new("Invalid bytecode. Attempt to Branch on non boolean"))
          }
        },
        Instruction::Jump{jump} => {
          index = Machine::calculate_jump(index, jump);
        }
        Instruction::Debug => {
          println!("Debug: \n  Stack: {:#?}\n  Locals: {:#?}\n  Function: ", &stack, &locals);
          func.debug(module)?;
        }

        _ => unimplemented!()
      }

      index += 1;
    }

    Err(SimpleError::new(format!("Overflowed function body")))
  }

  fn calculate_jump(index: usize, jump: i32) -> usize {
    if jump >= 0 {
      return index + (jump as usize);
    } else {
      let rel = (0 - jump) as usize;
      return index - rel;
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

struct ClosureFunction {
  func: Rc<RunFunction>,
  closures: Vec<Value>,
}

impl RunFunction for ClosureFunction {

  fn execute(&self, machine: &Machine, args: Vec<Value>) -> Result<Value, SimpleError> {
    let mut locals = self.closures.clone();
    let mut arg = args; // This is a weird hack I don't understand.
    locals.append(&mut arg);
    self.func.execute(machine, locals)
  }

  fn get_shape(&self) -> &Shape {
    &self.func.get_shape()
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
