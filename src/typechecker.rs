use std::collections::HashMap;

use simple_error::*;

use ast::*;
use shapes::*;

pub fn check_module(module: Module) -> Result<Module, SimpleError> {
  let mut exports = Vec::new();
  let mut locals = Vec::new();

  let mut scope = Scope::new();
  scope.create_function_scope();

  for ex in &module.exports {
    scope.pre_fill_module_function(&ex.content)?;
  }

  for ex in &module.locals {
    scope.pre_fill_module_function(ex)?;
  }

  for ex in module.exports {
    let loc = ex.loc.clone();
    if let Expression::FunctionDeclaration(content) = ex.content.check(&mut scope, shape_unknown())? {
      exports.push(Export { content: *content, loc });
    } else {
      return Err(SimpleError::new("FunctionDeclaration didn't return itself!"))
    }
  }

  for ex in module.locals {
    if let Expression::FunctionDeclaration(content) = ex.check(&mut scope, shape_unknown())? {
      locals.push(*content);
    } else {
      return Err(SimpleError::new("FunctionDeclaration didn't return itself!"))
    }
  }

  Ok(Module{exports, locals})
}

trait Typed {

  fn check(self, scope: &mut Scope, expected: Shape) -> Result<Expression, SimpleError>;

}

impl Typed for FunctionDeclarationEx {

  fn check(self, scope: &mut Scope, expected: Shape) -> Result<Expression, SimpleError> {
    let args = verify_function_declaration(self.args.clone(), expected, &self.loc)?;

    if !self.context.is_lambda {
      scope.set_scope(&self.id, &fill_shape(self.shape(), &self.loc)?, &self.loc)?;
    }

    scope.create_function_scope();

    for Parameter{id, shape} in &args {
      scope.set_scope(id, shape, &self.loc)?;
    }

    let body = check(scope, self.body, self.result.clone())?;

    let returned_shape = body.shape();

    let result = verify(self.result, returned_shape, &self.loc)?;

    let closures = scope.destroy_function_scope();

    Ok(FunctionDeclarationEx{result, body, id: self.id, args, loc: self.loc, context: self.context.set_closures(closures)}.wrap())
  }

}

impl Typed for BlockEx {

  fn check(self, scope: &mut Scope, expected: Shape) -> Result<Expression, SimpleError> {
    let BlockEx{shape: raw_shape, loc, body: raw_body} = self;
    let mut body = Vec::with_capacity(raw_body.len());

    if raw_body.len() == 0 {
      Ok(BlockEx{shape: shape_unit(), loc, body}.wrap())
    } else {
      scope.create_block_scope();

      let mut index = 0usize;
      let max = raw_body.len();
      for next in raw_body {
        index = index + 1;
        let expect = if max == index {
          expected.clone()
        } else {
          shape_unknown()
        };

        body.push(check(scope, next, expect)?);
      }
      let shape = body.last().expect("This shouldn't be possible!").shape();

      scope.destroy_block_scope();

      Ok(BlockEx{shape, loc, body}.wrap())
    }
  }
}

impl Typed for AssignmentEx {
  fn check(self, scope: &mut Scope, expected: Shape) -> Result<Expression, SimpleError> {
    let AssignmentEx{shape: raw_shape, id, loc, body: raw_body} = self;
    let body = check(scope, raw_body, raw_shape.clone())?;
    let shape = verify(raw_shape, body.shape(), &loc)?;

    scope.set_scope(&id, &shape, &loc)?;

    Ok(AssignmentEx{shape, id, loc, body}.wrap())
  }
}

impl Typed for BinaryOpEx {
  fn check(self, scope: &mut Scope, expected: Shape) -> Result<Expression, SimpleError> {
    let BinaryOpEx{shape: raw_shape, left: raw_left, right: raw_right, op, loc} = self;
    let left = check(scope, raw_left, shape_unknown())?;
    let right = check(scope, raw_right, shape_unknown())?;

    if left.shape() == right.shape() {
      let shape = verify(raw_shape, left.shape(), &loc)?;
      Ok(BinaryOpEx{shape, left, right, op, loc}.wrap())
    } else {
      Err(SimpleError::new(format!("Incompatible types! Cannot perform operation '{}' on distinct types '{}' and '{}' {}", op, left.shape().pretty(), right.shape().pretty(), loc.pretty())))
    }
  }
}

impl Typed for CallEx {
  fn check(self, scope: &mut Scope, expected: Shape) -> Result<Expression, SimpleError> {
    let CallEx{shape: raw_shape, loc, func: raw_func, args: raw_args} = self;
    let func = check(scope, raw_func, shape_unknown())?;

    if let Shape::SimpleFunctionShape {args: expected_args, result} = func.shape() {
      if raw_args.len() != expected_args.len() {
        return loc.fail("Incorrect number of arguments")?;
      }

      let mut args = Vec::new();

      for (expect, raw_arg) in expected_args.iter().zip(raw_args) {
        let arg = check(scope, raw_arg, expect.clone())?;

        if arg.shape() != *expect {
          return loc.fail("Invalid argument types for call")?;
        }

        args.push(arg);
      }

      Ok(CallEx {
        shape: *result,
        loc,
        func,
        args
      }.wrap())
    } else {
      return loc.fail("Attempt to call non-function");
    }
  }
}

impl Typed for VariableEx {
  fn check(self, scope: &mut Scope, expected: Shape) -> Result<Expression, SimpleError> {
    let VariableEx{shape: raw_shape, loc, id} = self;
    let shape = scope.check_scope(&id, &loc)?;

    Ok(VariableEx {shape, loc, id}.wrap())
  }
}

impl Typed for StringLiteralEx {
  fn check(self, scope: &mut Scope, expected: Shape) -> Result<Expression, SimpleError> {
    Ok(self.wrap())
  }
}

impl Typed for NumberLiteralEx {
  fn check(self, scope: &mut Scope, expected: Shape) -> Result<Expression, SimpleError> {
    Ok(self.wrap())
  }
}

fn check(scope: &mut Scope, ex: Expression, expected: Shape) -> Result<Expression, SimpleError> {
  match ex {
    Expression::FunctionDeclaration(ex) => ex.check(scope, expected),
    Expression::Block(ex) => ex.check(scope, expected),
    Expression::Assignment(ex) => ex.check(scope, expected),
    Expression::BinaryOp(ex) => ex.check(scope, expected),
    Expression::Call(ex) => ex.check(scope, expected),
    Expression::Variable(ex) => ex.check(scope, expected),
    Expression::StringLiteral(ex) => ex.check(scope, expected),
    Expression::NumberLiteral(ex) => ex.check(scope, expected),
  }
}

fn fill_shape(shape: Shape, loc: &Location) -> Result<Shape, SimpleError> {
  match shape {
    Shape::SimpleFunctionShape { args: raw_args, result: raw_result } => {
      let mut args: Vec<Shape> = Vec::new();

      for next_arg in raw_args {
        args.push(fill_shape(next_arg, loc)?);
      }

      let result = Box::new(fill_shape(*raw_result, loc)?);

      Ok(Shape::SimpleFunctionShape{args, result})
    }
    Shape::NamedShape { name } => {
      // TODO: check against custom declared types.
      match name.as_ref() {
        "String" => Ok(shape_string()),
        "Float" => Ok(shape_float()),
        "Unit" => Ok(shape_unit()),
        _ => Err(SimpleError::new(format!("Could not find type: {}, {}", name, loc.pretty())))
      }
    },
    Shape::BaseShape{..} => Ok(shape.clone()),
    Shape::UnknownShape => Ok(shape_unknown()),
  }
}

fn verify(defined: Shape, found: Shape, loc: &Location) -> Result<Shape, SimpleError> {
  if let Shape::UnknownShape = defined {
    if let Shape::UnknownShape = found {
      loc.fail("Unknown shape")
    } else {
      Ok(fill_shape(found, loc)?)
    }
  } else {
    if let Shape::UnknownShape = found {
      Ok(fill_shape(defined, loc)?)
    } else {
      let filled_defined = fill_shape(defined, loc)?;
      let filled_found = fill_shape(found, loc)?;

      if filled_defined == filled_found {
        Ok(filled_found)
      } else {
        loc.fail(&format!("Incompatible types! Declared: {}, but found: {}", filled_defined.pretty(), filled_found.pretty()))
      }
    }
  }
}

fn verify_function_declaration(parameters: Vec<Parameter>, expected: Shape, loc: &Location) -> Result<Vec<Parameter>, SimpleError> {
  let expected_args = if let Shape::SimpleFunctionShape{args: expected_args, ..} = expected {
    expected_args.clone()
  } else {
    vec![shape_unknown(); parameters.len()]
  };

  let mut filled_args = Vec::new();

  for (arg, expected_arg) in parameters.iter().zip(expected_args) {
    let verified = verify(expected_arg, arg.shape.clone(), &loc)?;
    filled_args.push( Parameter{id: arg.id.clone(), shape: verified});
  }

  Ok(filled_args)
}


struct Scope {
  static_scope: HashMap<String, Shape>,
  block_stack: Vec<Vec<HashMap<String, Shape>>>,
  closures: Vec<Vec<String>>,
}

impl Scope {

  fn new() -> Scope {
    Scope{
      static_scope: HashMap::new(),
      block_stack: Vec::new(),
      closures: Vec::new(),
    }
  }

  fn pre_fill_module_function(&mut self, func: &FunctionDeclarationEx) -> Result<(), SimpleError> {
    let shape = fill_shape(func.shape(), &func.loc)?;

    self.static_scope.insert(func.id.clone(), shape);
    Ok(())
  }

  fn set_scope(&mut self, id: &String, shape: &Shape, loc: &Location) -> Result<(), SimpleError> {
    let block_scope = self.block_stack.last_mut().expect("Scope should never be empty!");
    let scope = block_scope.last_mut().expect("Block Scope should never be empty!");

    if scope.contains_key(id) {
      Err(SimpleError::new(format!("Redeclaration of variable: {} {}", id, loc.pretty())))
    } else {
      scope.insert(id.clone(), shape.clone());
      Ok(())
    }
  }

  fn check_scope(&mut self, id: &String, loc: &Location) -> Result<Shape, SimpleError> {
    let mut first = true;

    for block_scope in self.block_stack.iter().rev() {
      for scope in block_scope {
        if scope.contains_key(id) {
          if !first {
            self.closures.last_mut().expect("closures should never be empty!").push(id.clone());
          }

          return Ok(scope[id].clone());
        }
      }

      first = false;
    }

    if self.static_scope.contains_key(id) {
      return Ok(self.static_scope[id].clone())
    }

    Err(SimpleError::new(format!("Undeclared variable: {} {}", id, loc.pretty())))
  }

  fn create_block_scope(&mut self) {
    self.block_stack.last_mut().expect("Block Scope should never be empty!").push(HashMap::new());
  }

  fn destroy_block_scope(&mut self) {
    self.block_stack.last_mut().expect("Block Scope should never be empty!").pop();
  }

  fn create_function_scope(&mut self) {
    self.block_stack.push(vec![HashMap::new()]);
    self.closures.push(Vec::new());
  }

  fn destroy_function_scope(&mut self) -> Vec<String> {
    self.block_stack.pop();
    self.closures.pop()
      .expect("closures should never be empty!")
  }
}
