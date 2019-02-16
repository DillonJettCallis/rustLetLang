use std::collections::HashMap;
use std::rc::Rc;

use bytebuffer::ByteBuffer;

use bytecode::*;
use shapes::*;

use simple_error::SimpleError;
use runtime::Value;

pub trait RunFunction {

  fn execute(&self, machine: &Machine, args: &Vec<Value>) -> Result<Value, SimpleError>;

}

pub struct Machine {
  app: AppDirectory,
  core_functions: Vec<Box<RunFunction>>,
}

impl Machine {

  pub fn new(app: AppDirectory) -> Machine {
    let mut core_functions: Vec<Box<RunFunction>> = Vec::new();
    core_functions.push(Box::new(sum_impl()));
    Machine{app, core_functions}
  }

  pub fn execute(&self, func: &BitFunction, args: &Vec<Value>) -> Result<Value, SimpleError> {
    let mut index = 0usize;
    let mut stack: Vec<Value> = Vec::new();
    let mut locals: Vec<Value> = Vec::with_capacity(func.max_locals as usize);

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
        Instruction::LoadConst{kind, const_id} => {
          match kind {
            0 => { // String
              let index = const_id as usize;

              let value: &String = self.app.string_constants.get(index)
                .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid String constant id"))?;

              stack.push(Value::String(Rc::new(value.clone())));
            },
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
        Instruction::CallBuiltIn{func_id, shape_id} => {
          let func: &Box<RunFunction> = self.core_functions.get(func_id as usize)
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid built in function id"))?;

          let shape: &Shape = self.app.shape_refs.get(shape_id as usize)
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid Shape constant"))?;

          if let Shape::SimpleFunctionShape{args, result: _} = shape {
            let size = args.len();
            let mut params: Vec<Value> = Vec::with_capacity(size);

            for i in 0..size {
              let param = stack.pop()
                .ok_or_else(|| SimpleError::new("Invalid bytecode. Not enough args for function"))?;

              params.push(param);
            }

            let result = func.execute(&self, &params)?;
            stack.push(result);
          } else {
            return Err(SimpleError::new("Invalid bytecode. CallBuiltIn is not function"))
          }
        },
        Instruction::CallStatic{func_id, shape_id} => {
          let func: &BitFunction = self.app.function_refs.get(func_id as usize)
            .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid built in function id"))?;

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

            let result = func.execute(&self, &params)?;
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

            let maybe_func: Value = stack.pop()
              .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid built in function id"))?;

            if let Value::Function(func) = maybe_func {
              let result = func.execute(&self, &params)?;
              stack.push(result);
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
        }

        _ => unimplemented!()
      }

      index += 1;
    }

    Err(SimpleError::new(format!("Overflowed function body")))
  }

}

impl RunFunction for BitFunction {

  fn execute(&self, machine: &Machine, args: &Vec<Value>) -> Result<Value, SimpleError> {
    machine.execute(self, args)
  }

}

fn sum_impl() -> impl RunFunction {
  struct SumFun{}

  impl RunFunction for SumFun {
    fn execute(&self, machine: &Machine, args: &Vec<Value>) -> Result<Value, SimpleError> {
      if args.len() == 2 {
        if let Value::Float(first) = args[0] {
          if let Value::Float(second) = args[1] {
            return Ok(Value::Float(first + second));
          }
        }
      }

      return Err(SimpleError::new("Core.+ takes exactly two float arguments"));
    }
  }

  return SumFun{}
}

