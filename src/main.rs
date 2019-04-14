extern crate bytebuffer;
extern crate core;
extern crate simple_error;

use std::collections::HashMap;

use simple_error::SimpleError;

use ast::Module;
use bytecode::{BitApplication, BitModule, BitPackage};
use bytecode::BitFunction;
use bytecode::FunctionRef;
use bytecode::Instruction;
use compiler::compile_package;
use interpreter::Machine;
use interpreter::RunFunction;
use runtime::Value;
use shapes::BaseShapeKind;
use shapes::Shape;
use parser::parse;
use std::path::Path;
use typechecker::check_module;
use ir::compile_ir_module;

mod shapes;
mod ast;
mod parser;
mod typechecker;
mod bytecode;
mod interpreter;
mod runtime;
mod compiler;
mod optimize;
mod ir;

fn main() {
  match ir_compile_test() {
    Ok(Value::Float(result)) => println!("Success: \n{:#?}", result),
    Ok(_) => println!("Failure: "),
    Err(simple_error) => println!("Error: {}", simple_error.as_str())
  }
}

fn compile_test() -> Result<Value, SimpleError> {
  let module_name = String::from("basic");
  let package_name = String::from("test");

  let package = compile_package("test", "/home/dillon/projects/rustLetLang/test")?;
  let mut app = BitApplication::new(package_name.clone(), module_name);
  app.packages.insert(package_name, package);

  let machine = Machine::new(app);

  machine.run_main()
}

fn ir_compile_test() -> Result<Value, SimpleError> {

  let parsed = parse(&Path::new("/home/dillon/projects/rustLetLang/test/basic.let"), "test", "basic")?;
  let checked = check_module(parsed)?;
  let compiled = compile_ir_module(&checked)?;

  compiled.debug()?;

  Ok(Value::Float(10f64))
}
