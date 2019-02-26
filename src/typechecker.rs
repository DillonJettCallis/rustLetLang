use std::collections::HashMap;

use simple_error::*;

use ast::*;
use shapes::*;

// min error
// Err(SimpleError::new(""))

pub fn check_module(module: Module) -> Result<Module, SimpleError> {
  let mut exports: Vec<Export> = Vec::new();
  let mut locals: Vec<Box<Expression>> = Vec::new();

  let mut scope = Scope::new();
  scope.create_function_scope();

  for ref ex in &module.exports {
    scope.pre_fill_module_function(&ex.content)?;
  }

  for ref ex in &module.locals {
    scope.pre_fill_module_function(&ex)?;
  }

  for ex in module.exports {
    let loc = ex.loc.clone();
    let content = Box::new(check(&mut scope, *ex.content)?);

    exports.push(Export{content, loc});
  }

  for ex in module.locals {
    let content = check(&mut scope, *ex)?;

    locals.push(Box::new(content));
  }

  Ok(Module{exports, locals})
}


fn check(scope: &mut Scope, ex: Expression) -> Result<Expression, SimpleError> {
  match ex {
    Expression::FunctionDeclaration{shape: raw_shape, loc, id, args, body: raw_body, ..} => {
      let (arg_shapes, result_shape) = verify_function_declaration(&raw_shape, &args, &loc)?;

      if id != "<anon>" {
        scope.set_scope(&id, &fill_shape(&raw_shape, &loc)?, &loc)?;
      }

      scope.create_function_scope();

      for (arg_id, arg_shape) in args.iter().zip(arg_shapes.iter()) {
        scope.set_scope(arg_id, arg_shape, &loc)?;
      }

      let body = check(scope, *raw_body)?;

      let returned_shape = body.shape().clone();

      let final_result_shape = verify(result_shape, returned_shape, &loc)?;

      let closures = scope.destroy_function_scope();

      let shape = Shape::SimpleFunctionShape {args: arg_shapes, result: Box::new(final_result_shape)};

      Ok(Expression::FunctionDeclaration{shape, body: Box::new(body), id, args, loc, closures})
    }
    Expression::Block{shape: raw_shape, loc, body: raw_body} => {
      let mut body: Vec<Box<Expression>> = Vec::with_capacity(raw_body.len());

      if raw_body.len() == 0 {
        Ok(Expression::Block{shape: shape_unit(), loc, body})
      } else {
        scope.create_block_scope();

        for next in raw_body {
          body.push(Box::new(check(scope, *next)?));
        }
        let shape = body.last().expect("This shouldn't be possible!").shape().clone();

        scope.destroy_block_scope();

        Ok(Expression::Block{shape, loc, body})
      }
    }
    Expression::Assignment{shape: raw_shape, id, loc, body: raw_body} => {
      let body = check(scope, *raw_body)?;
      let shape = verify(raw_shape, body.shape().clone(), &loc)?;

      scope.set_scope(&id, &shape, &loc)?;

      Ok(Expression::Assignment{shape, id, loc, body: Box::new(body)})
    }
    Expression::BinaryOp{shape: raw_shape, left: raw_left, right: raw_right, op, loc} => {
      let left = check(scope, *raw_left)?;
      let right = check(scope, *raw_right)?;

      if left.shape() == right.shape() {
        let shape = verify(raw_shape, left.shape().clone(), &loc)?;
        Ok(Expression::BinaryOp{shape, left: Box::new(left), right: Box::new(right), op, loc})
      } else {
        Err(SimpleError::new(format!("Incompatible types! Cannot perform operation '{}' on distinct types '{}' and '{}' {}", op, left.shape().pretty(), right.shape().pretty(), loc.pretty())))
      }
    },
    Expression::Call {shape: raw_shape, loc, func: raw_func, args: raw_args} => {
      let func = check(scope, *raw_func)?;
      let mut args = Vec::new();

      for raw_arg in raw_args {
        args.push(Box::new(check(scope, *raw_arg)?));
      }

      if let Shape::SimpleFunctionShape {args: expected_args, result} = func.shape().clone() {
        if args.len() != expected_args.len() {
          return loc.fail("Incorrect number of arguments")?;
        }

        for index in 0..args.len() {
          if args[index].shape() != &expected_args[index] {
            return loc.fail("Invalid argument types for call")?;
          }
        }

        Ok(Expression::Call {
          shape: *result,
          loc,
          func: Box::new(func),
          args
        })
      } else {
        return loc.fail("Attempt to call non-function");
      }
    },
    Expression::Variable{shape: raw_shape, loc, id} => {
      let shape = scope.check_scope(&id, &loc)?;

      Ok(Expression::Variable {shape, loc, id})
    }
    Expression::StringLiteral{..} => Ok(ex),
    Expression::NumberLiteral{..} => Ok(ex)
  }
}

fn fill_shape(shape: &Shape, loc: &Location) -> Result<Shape, SimpleError> {
  match shape {
    Shape::SimpleFunctionShape { args: raw_args, result: raw_result } => {
      let mut args: Vec<Shape> = Vec::new();

      for next_arg in raw_args {
        args.push(fill_shape(next_arg, loc)?);
      }

      let result = Box::new(fill_shape(raw_result, loc)?);

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
    _ => loc.fail("Unknown shape"),
  }
}

fn verify(defined: Shape, found: Shape, loc: &Location) -> Result<Shape, SimpleError> {
  if let Shape::UnknownShape = defined {
    Ok(found)
  } else {
    let filled_defined = fill_shape(&defined, loc)?;

    if filled_defined == found {
      Ok(found)
    } else {
      Err(SimpleError::new(format!("Incompatible types! Declared: {}, but found: {}, {}", filled_defined.pretty(), found.pretty(), loc.pretty())))
    }
  }
}

fn verify_function_declaration(defined: &Shape, arg_ids: &Vec<String>, loc: &Location) -> Result<(Vec<Shape>, Shape), SimpleError> {
  if let Shape::SimpleFunctionShape {args, result} = defined {
    if args.len() != arg_ids.len() {
      Err(SimpleError::new( format!("Incompatible types! Function type has different number of parameters than named arguments. Type: {}, args found: {} {}", defined.pretty(), arg_ids.len(), loc.pretty())))
    } else {
      let mut filled_args = Vec::new();

      for arg in args {
        filled_args.push(fill_shape(arg, loc)?);
      }

      Ok( (filled_args, *result.clone()) )
    }
  } else {
    Err(SimpleError::new( format!("Function has type that is not a function type! Declared type: '{}' {}", defined.pretty(), loc.pretty())))
  }
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

  fn pre_fill_module_function(&mut self, ex: &Expression) -> Result<(), SimpleError> {
    if let Expression::FunctionDeclaration { shape, loc, id, .. } = ex {
      let shape = fill_shape(ex.shape(), &ex.loc())?;

      self.static_scope.insert(id.clone(), shape);
      Ok(())
    } else {
      Err(SimpleError::new(format!("Invalid function declaration: {}", ex.loc().pretty())))
    }
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
