extern crate bytebuffer;
extern crate simple_error;

use std::collections::HashMap;

use simple_error::SimpleError;

use ast::Module;
use bytecode::{BitModule, BitPackage, BitApplication};
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
  let module_name = String::from("util");
  let package_name = String::from("test");

  let package = Compiler::compile_package("test", "/home/dillon/projects/rustLetLang/test")?;
  let mut app = BitApplication::new(package_name.clone(), module_name);
  app.packages.insert(package_name, package);

  let machine = Machine::new(app);

  machine.run_main()
}
