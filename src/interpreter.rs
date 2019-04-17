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
use lib_core::core_runtime;

pub enum RunFunction {
  BitFunction(BitFunction),
  NativeFunction(NativeFunction),
}


pub trait FunctionHandle {
  fn with(&self, args: Vec<Value>) -> (&FunctionRef, Vec<Value>);
}

impl Debug for FunctionHandle {
  fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
    f.write_str("<function>")
  }
}

pub struct Machine {
  app: BitApplication,
}

impl Machine {
  pub fn new(mut app: BitApplication) -> Machine {
    app.packages.insert(String::from("Core"), core_runtime());
    Machine { app }
  }

  pub fn run_main(&self) -> Result<Value, SimpleError> {
    self.execute(self.app.main.clone(), vec![])
  }

  pub fn execute(&self, mut src_func_ref: FunctionRef, mut locals: Vec<Value>) -> Result<Value, SimpleError> {
    'outer: loop {
      match self.app.lookup_function(&src_func_ref)? {
        RunFunction::BitFunction(func) => {
          let module = self.app.lookup_module(&src_func_ref)?;

          let mut index = 0usize;
          let mut stack: Vec<Value> = Vec::new();
          locals.resize(func.max_locals as usize, Value::Null);

          while index < func.body.len() {
            match func.body[index] {
              Instruction::NoOp => {}
              Instruction::Duplicate => {
                let last = stack.last()
                  .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to duplicate empty stack"))?
                  .clone();
                stack.push(last);
              }
              Instruction::Pop => {
                stack.pop()
                  .ok_or_else(|| SimpleError::new("Invalid bytecode in module. Attempt to pop empty stack"))?;
              }
              Instruction::Swap => {
                let first = stack.pop()
                  .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to swap empty stack"))?;

                let second = stack.pop()
                  .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to swap stack of 1"))?;

                stack.push(first);
                stack.push(second);
              }
              Instruction::LoadConstNull => {
                stack.push(Value::Null);
              }
              Instruction::LoadConstTrue => {
                stack.push(Value::True);
              }
              Instruction::LoadConstFalse => {
                stack.push(Value::False);
              }
              Instruction::LoadConstString { const_id } => {
                stack.push(Value::String(Rc::new(module.lookup_string(const_id)?)));
              }
              Instruction::LoadConstFunction { const_id } => {
                let func_ref = module.lookup_function(const_id)?;

                stack.push(Value::Function(Rc::new(func_ref)));
              }
              Instruction::LoadConstFloat { value } => stack.push(Value::Float(value)),
              Instruction::LoadValue { local } => {
                let index = local as usize;

                let local: &Value = locals.get(index)
                  .ok_or_else(|| SimpleError::new("Invalid bytecode. LoadValue of local that doesn't exist"))?;

                stack.push(local.clone());
              }
              Instruction::StoreValue { local } => {
                let index = local as usize;

                let value = stack.pop()
                  .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to StoreValue of empty stack"))?;

                locals[index] = value;
              }
              Instruction::CallStatic { func_id } => {
                let func_ref = module.function_refs.get(func_id as usize)
                  .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid function id"))?
                  .clone();

                if let Shape::SimpleFunctionShape { args, result: _ } = func_ref.shape.clone() {
                  let size = args.len();
                  let mut params: Vec<Value> = Vec::with_capacity(size);

                  for i in 0..size {
                    let param = stack.pop()
                      .ok_or_else(|| SimpleError::new("Invalid bytecode. Not enough args for function"))?;

                    params.push(param);
                  }

                  params.reverse();

                  if let Instruction::Return = func.body[index + 1] {
                    src_func_ref = func_ref;
                    locals = params;
                    continue 'outer;
                  } else {
                    let result = self.execute(func_ref, params)?;
                    stack.push(result);
                  }
                } else {
                  return Err(SimpleError::new("Invalid bytecode. CallStatic is not function"));
                }
              }
              Instruction::CallDynamic { param_count } => {
                let mut params: Vec<Value> = Vec::with_capacity(param_count as usize);

                for i in 0..param_count {
                  let param = stack.pop()
                    .ok_or_else(|| SimpleError::new("Invalid bytecode. Not enough args for function"))?;

                  params.push(param);
                }

                params.reverse();

                let maybe_func: Value = stack.pop()
                  .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid built in function id"))?;

                if let Value::Function(handle) = maybe_func {
                  let (func_ref, new_locals) = handle.with(params);

                  if let Instruction::Return = func.body[index + 1] {
                    src_func_ref = func_ref.clone();
                    locals = new_locals;
                    continue 'outer;
                  }

                  let result = self.execute(func_ref.clone(), new_locals)?;
                  stack.push(result);
                } else {
                  return Err(SimpleError::new("Invalid bytecode. CallDynamic is not function"));
                }
              }
              Instruction::BuildClosure { param_count, func_id } => {
                let func = module.function_refs.get(func_id as usize)
                  .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid function id"))?;

                let mut params = Vec::with_capacity(param_count as usize);

                for _ in 0..param_count {
                  let param = stack.pop()
                    .ok_or_else(|| SimpleError::new("Invalid bytecode. Not enough args for closure"))?;
                  params.push(param);
                }

                params.reverse();

                let closure = ClosureHandle {
                  func: func.clone(),
                  closures: params,
                };

                stack.push(Value::Function(Rc::new(closure)));
              }
              Instruction::BuildRecursiveFunction => {
                let maybe_func = stack.pop().ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to BuildRecursiveFunction of empty stack"))?;

                if let Value::Function(func) = maybe_func {
                  stack.push(Value::Function(Rc::new(RecursiveHandle { func })));
                } else {
                  return Err(SimpleError::new("Invalid bytecode. BuildRecursiveFunction is not function"));
                }
              }
              Instruction::Return => {
                return stack.pop()
                  .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to return empty stack"));
              }
              Instruction::Branch { jump } => {
                let first = stack.pop()
                  .ok_or_else(|| SimpleError::new("Invalid bytecode. Attempt to Branch empty stack"))?;

                match first {
                  Value::True => {}
                  Value::False => index = Machine::calculate_jump(index, jump),
                  _ => return Err(SimpleError::new("Invalid bytecode. Attempt to Branch on non boolean"))
                }
              }
              Instruction::Jump { jump } => {
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

          return Err(SimpleError::new(format!("Overflowed function body")));
        }
        RunFunction::NativeFunction(native) => {
          return (native.func)(self, locals);
        }
      }
    }
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

impl FunctionHandle for FunctionRef {
  fn with(&self, args: Vec<Value>) -> (&FunctionRef, Vec<Value>) {
    (&self, args)
  }
}

struct ClosureHandle {
  func: FunctionRef,
  closures: Vec<Value>,
}

impl FunctionHandle for ClosureHandle {
  fn with(&self, mut args: Vec<Value>) -> (&FunctionRef, Vec<Value>) {
    let mut locals = self.closures.clone();
    locals.append(&mut args);
    (&self.func, locals)
  }
}

struct RecursiveHandle {
  func: Rc<FunctionHandle>,
}

impl FunctionHandle for RecursiveHandle {
  fn with(&self, mut args: Vec<Value>) -> (&FunctionRef, Vec<Value>) {
    let mut locals = Vec::with_capacity(args.len() + 1);
    locals.push(Value::Function(Rc::new(RecursiveHandle { func: self.func.clone() })));
    locals.append(&mut args);
    self.func.with(locals)
  }
}

pub struct NativeFunction {
  pub func: Box<Fn(&Machine, Vec<Value>) -> Result<Value, SimpleError>>,
  pub func_ref: FunctionRef,
}

