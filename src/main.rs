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
    Ok(Value::Float(result)) => print!("Success: \n{:#?}", result),
    Ok(_) => print!("Failure: "),
    Err(simple_error) => print!("Error: {}", simple_error.as_str())
  }
}

fn parse_test() -> Result<Module, SimpleError> {
  let parsed = parser::parse("/home/dillon/projects/rustLetLang/test/basic.let")?;
  typechecker::check_module(parsed)
}

fn execute_test() -> Result<Value, SimpleError> {
  let mut body: Vec<Instruction> = Vec::new();

  body.push(Instruction::LoadConstFloat {value: 5.0});
  body.push(Instruction::LoadConstFloat {value: 2.0});
  body.push(Instruction::CallBuiltIn {func_id: 0, shape_id: 0});
  body.push(Instruction::Return);

  let main = BitFunction {
    max_locals: 0,
    function_shape: Shape::SimpleFunctionShape {
      args: vec![],
      result: Box::new(Shape::BaseShape {kind: BaseShapeKind::Float})
    },
    body,
    source: vec![]
  };

  let mut shape_refs: Vec<Shape> = Vec::new();
  shape_refs.push(Shape::SimpleFunctionShape {
    args: vec![Shape::BaseShape {kind: BaseShapeKind::Float}, Shape::BaseShape {kind: BaseShapeKind::Float}],
    result: Box::new(Shape::BaseShape {kind: BaseShapeKind::Float})
  });

  let app = AppDirectory {
    string_constants: vec![],
    function_refs: vec![],
    shape_refs,
    source: String::new()
  };

  let machine = Machine::new(app);

  machine.execute(&main, &vec![])
}
