extern crate simple_error;
extern crate bytebuffer;

mod shapes;
mod ast;
mod parser;
mod typechecker;
mod bytecode;
mod interpreter;
mod runtime;
mod compiler;

use ast::Module;
use simple_error::SimpleError;
use bytecode::Instruction;
use runtime::Value;
use shapes::Shape;
use shapes::BaseShapeKind;
use bytecode::AppDirectory;
use interpreter::Machine;
use bytecode::BitFunction;
use std::collections::HashMap;
use bytecode::FunctionRef;
use interpreter::RunFunction;
use compiler::Compiler;

fn main() {
  match compile_test() {
    Ok(Value::Float(result)) => println!("Success: \n{:#?}", result),
    Ok(_) => println!("Failure: "),
    Err(simple_error) => println!("Error: {}", simple_error.as_str())
  }
}

fn compile_test() -> Result<Value, SimpleError> {
  let parsed = parser::parse("/home/dillon/projects/rustLetLang/test/basic.let")?;
  let checked = typechecker::check_module(parsed)?;
  let compiled = Compiler::compile(checked)?;
  let machine = Machine::new(compiled);

  machine.run_main()
}

fn parse_test() -> Result<Module, SimpleError> {
  let parsed = parser::parse("/home/dillon/projects/rustLetLang/test/basic.let")?;
  typechecker::check_module(parsed)
}

fn execute_test() -> Result<Value, SimpleError> {
  let main = build_main();

  let op_shape = Shape::SimpleFunctionShape {
    args: vec![Shape::BaseShape {kind: BaseShapeKind::Float}, Shape::BaseShape {kind: BaseShapeKind::Float}],
    result: Box::new(Shape::BaseShape {kind: BaseShapeKind::Float})
  };

  let mut functions: HashMap<String, Box<RunFunction>> = HashMap::new();
  functions.insert(String::from("Basic.pow"), Box::new(build_pow()));

  let app = AppDirectory {
    string_constants: vec![],
    function_refs: vec![
      FunctionRef{name: String::from("Core.+"), shape:  op_shape.clone()},
      FunctionRef{name: String::from("Core.*"), shape:  op_shape.clone()},
      FunctionRef{name: String::from("Basic.pow"), shape:  op_shape.clone()}
    ],
    functions,
    shape_refs: vec![op_shape.clone()]
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

  // while (power != 0) {
  body.push(Instruction::LoadConstFloat {value: 0.0}); // load zero for check
  body.push(Instruction::LoadValue {local: 1}); // load from argument 1, the power
  body.push(Instruction::IfEqual{jump: 8}); // check if power remaining is zero


  //   i = i * base;
  body.push(Instruction::LoadValue {local: 0}); // load base to stack
  body.push(Instruction::CallStatic {func_id: 1}); // call Core.*
  //   power = power - 1;
  body.push(Instruction::LoadValue {local: 1}); // load counter to stack
  body.push(Instruction::LoadConstFloat {value: -1.0}); // load negative one to subtract
  body.push(Instruction::CallStatic {func_id: 0}); // call Core.+ with -1 to subtract one
  body.push(Instruction::StoreValue {local: 1}); // counter has been decremented, store it back in local
  body.push(Instruction::Jump {jump: -9}); // jump back up to the loop
  // }

  // return i;
  body.push(Instruction::Return); // return answer

  return BitFunction {
    max_locals: 2,
    shape: Shape::SimpleFunctionShape {
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
  body.push(Instruction::CallStatic {func_id: 2});
  body.push(Instruction::Return);

  return BitFunction {
    max_locals: 0,
    shape: Shape::SimpleFunctionShape {
      args: vec![],
      result: Box::new(Shape::BaseShape {kind: BaseShapeKind::Float})
    },
    body,
    source: vec![]
  };
}

