extern crate bytebuffer;
extern crate simple_error;

use std::collections::HashMap;

use simple_error::SimpleError;

use ast::Module;
use bytecode::BitModule;
use bytecode::BitFunction;
use bytecode::FunctionRef;
use bytecode::Instruction;
use compiler::Compiler;
use interpreter::Machine;
use interpreter::RunFunction;
use runtime::Value;
use shapes::BaseShapeKind;
use shapes::Shape;

mod shapes;
mod ast;
mod parser;
mod typechecker;
mod bytecode;
mod interpreter;
mod runtime;
mod compiler;

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
