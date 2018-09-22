extern crate simple_error;

mod shapes;
mod ast;
mod parser;
mod typechecker;

use ast::Module;
use simple_error::SimpleError;

fn main() {
  match run_safely() {
    Ok(ast) => print!("Success: \n{:#?}", ast),
    Err(simple_error) => print!("Error: {}", simple_error.as_str())
  }
}

fn run_safely() -> Result<Module, SimpleError> {
  let parsed = parser::parse("/home/dillon/projects/rustLetLang/test/basic.let")?;
  typechecker::check_module(parsed)
}
