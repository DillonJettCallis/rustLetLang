extern crate core;
extern crate simple_error;
extern crate serde;
extern crate bincode;

use std::collections::HashMap;
use std::path::Path;

use simple_error::SimpleError;

use ast::Module;
use bytecode::{BitApplication, BitModule, BitPackage};
use bytecode::BitFunction;
use bytecode::FunctionRef;
use bytecode::Instruction;
use compiler::compile_package;
use interpreter::Machine;
use interpreter::RunFunction;
use ir::compile_ir_module;
use parser::parse;
use runtime::Value;
use shapes::{BaseShapeKind, shape_unknown, shape_float};
use shapes::Shape;
use typechecker::check_module;

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
mod lib_core;

fn main() {
  match compile_test() {
    Ok(Value::Float(result)) => println!("Success: \n{:#?}", result),
    Ok(_) => println!("Failure: "),
    Err(simple_error) => println!("Error: {}", simple_error.as_str())
  }
}

fn compile_test() -> Result<Value, SimpleError> {
  let module_name = String::from("basic");
  let package_name = String::from("test");

  let package = compile_package("test", "/home/dillon/projects/rustLetLang/test")?;
  let mut app = BitApplication::new(FunctionRef {
    package: package_name.clone(),
    module: module_name.clone(),
    name: String::from("main"),

    shape: Shape::SimpleFunctionShape {
      args: vec![],
      result: Box::new(shape_float()),
    }
  });
  app.packages.insert(package_name, package);

  let machine = Machine::new(app);

  machine.run_main()
}
