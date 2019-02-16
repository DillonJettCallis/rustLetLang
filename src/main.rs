extern crate simple_error;
extern crate bytebuffer;

mod shapes;
mod ast;
mod parser;
mod typechecker;
mod bytecode;
mod interpreter;
mod runtime;

use ast::Module;
use simple_error::SimpleError;
use bytecode::Instruction;
use runtime::Value;
use shapes::Shape;
use shapes::BaseShapeKind;
use bytecode::AppDirectory;
use interpreter::Machine;
use bytecode::BitFunction;

fn main() {
  match execute_test() {
    Ok(Value::Float(result)) => println!("Success: \n{:#?}", result),
    Ok(_) => println!("Failure: "),
    Err(simple_error) => println!("Error: {}", simple_error.as_str())
  }
}

fn parse_test() -> Result<Module, SimpleError> {
  let parsed = parser::parse("/home/dillon/projects/rustLetLang/test/basic.let")?;
  typechecker::check_module(parsed)
}

fn execute_test() -> Result<Value, SimpleError> {
  let main = build_main();

  let mut shape_refs: Vec<Shape> = Vec::new();
  shape_refs.push(Shape::SimpleFunctionShape {
    args: vec![Shape::BaseShape {kind: BaseShapeKind::Float}, Shape::BaseShape {kind: BaseShapeKind::Float}],
    result: Box::new(Shape::BaseShape {kind: BaseShapeKind::Float})
  });

  let app = AppDirectory {
    string_constants: vec![],
    function_refs: vec![build_pow()],
    shape_refs,
    source: String::new()
  };

  let machine = Machine::new(app);

  machine.execute(&main, vec![])
}

fn build_pow() -> BitFunction {
  let float_shape = Shape::BaseShape {kind: BaseShapeKind::Float};

  // fun pow(base: Float, power: Int) {
  let mut body: Vec<Instruction> = Vec::new();
  // let i = 1;
  body.push(Instruction::LoadConstFloat {value: 1.0}); // Init answer
  body.push(Instruction::StoreValue {local: 2}); // store answer in empty third local

  // while (power != 0) {
  body.push(Instruction::LoadConstFloat {value: 0.0}); // load zero for check
  body.push(Instruction::LoadValue {local: 1}); // load from argument 1, the power
  body.push(Instruction::IfEqual{jump: 10}); // check if power remaining is zero


  //   i = i * base;
  body.push(Instruction::LoadValue {local: 2}); // load answer to stack
  body.push(Instruction::LoadValue {local: 0}); // load base to stack
  body.push(Instruction::CallBuiltIn {func_id: 1, shape_id: 0}); // call Core.*
  body.push(Instruction::StoreValue {local: 2}); // store to answer
  //   power = power - 1;
  body.push(Instruction::LoadValue {local: 1}); // load counter to stack
  body.push(Instruction::LoadConstFloat {value: -1.0}); // load negative one to subtract
  body.push(Instruction::CallBuiltIn {func_id: 0, shape_id: 0}); // call Core.+ with -1 to subtract one
  body.push(Instruction::StoreValue {local: 1}); // counter has been decremented, store it back in local
  body.push(Instruction::Jump {jump: -11}); // jump back up to the loop
  // }

  // return i;
  body.push(Instruction::LoadValue {local: 2}); // load answer to stack
  body.push(Instruction::Return); // return answer

  return BitFunction {
    max_locals: 3,
    function_shape: Shape::SimpleFunctionShape {
      args: vec![float_shape.clone(), float_shape.clone()],
      result: Box::new(float_shape)
    },
    body,
    source: vec![]
  };
}

fn build_main() -> BitFunction {
  let mut body: Vec<Instruction> = Vec::new();

  body.push(Instruction::LoadConstFloat {value: 5.0});
  body.push(Instruction::LoadConstFloat {value: 3.0});
  body.push(Instruction::CallStatic {func_id: 0, shape_id: 0});
  body.push(Instruction::Return);

  return BitFunction {
    max_locals: 0,
    function_shape: Shape::SimpleFunctionShape {
      args: vec![],
      result: Box::new(Shape::BaseShape {kind: BaseShapeKind::Float})
    },
    body,
    source: vec![]
  };
}

