use std::collections::HashMap;

use simple_error::*;

use ast::*;
use shapes::*;

type Scope = Vec<HashMap<String, Shape>>;

// min error
// Err(SimpleError::new(""))

pub fn check_module(module: Module) -> Result<Module, SimpleError> {
  let mut exports: Vec<Export> = Vec::new();
  let mut locals: Vec<Box<Expression>> = Vec::new();

  let mut scope_stack: Vec<HashMap<String, Shape>> = Vec::new();
  create_scope(&mut scope_stack);

  for ref ex in &module.exports {
    pre_fill_module_function(&mut scope_stack, &ex.content)?;
  }

  for ref ex in &module.locals {
    pre_fill_module_function(&mut scope_stack, &ex)?;
  }

  for ex in module.exports {
    let loc = ex.loc.clone();
    let content = Box::new(check(&mut scope_stack, *ex.content)?);

    exports.push(Export{content, loc});
  }

  for ex in module.locals {
    let content = check(&mut scope_stack, *ex)?;

    locals.push(Box::new(content));
  }

  Ok(Module{exports, locals})
}


fn check(scope: &mut Scope, ex: Expression) -> Result<Expression, SimpleError> {
  match ex {
    Expression::FunctionDeclaration{shape: raw_shape, loc, id, args, body: raw_body} => {
      let filled_shape = fill_shape(raw_shape, &loc)?;
      let (arg_shapes, result_shape) = verify_function_declaration(&filled_shape, &args, &loc)?;

      create_scope(scope);

      for (arg_id, arg_shape) in args.iter().zip(arg_shapes.iter()) {
        set_scope(scope, arg_id, arg_shape, &loc)?;
      }

      let body = check(scope, *raw_body)?;

      let returned_shape = body.shape().clone();

      let final_result_shape = verify(result_shape, returned_shape, &loc)?;

      destroy_scope(scope);

      let shape = Shape::SimpleFunctionShape {args: arg_shapes, result: Box::new(final_result_shape)};

      Ok(Expression::FunctionDeclaration{shape, body: Box::new(body), id, args, loc})
    }
    Expression::Block{shape: raw_shape, loc, body: raw_body} => {
      let mut body: Vec<Box<Expression>> = Vec::with_capacity(raw_body.len());

      if raw_body.len() == 0 {
        Ok(Expression::Block{shape: shape_unit(), loc, body})
      } else {
        create_scope(scope);

        for next in raw_body {
          body.push(Box::new(check(scope, *next)?));
        }
        let shape = body.last().expect("This shouldn't be possible!").shape().clone();

        destroy_scope(scope);

        Ok(Expression::Block{shape, loc, body})
      }
    }
    Expression::Assignment{shape: raw_shape, id, loc, body: raw_body} => {
      let body = check(scope, *raw_body)?;
      let shape = verify(raw_shape, body.shape().clone(), &loc)?;

      set_scope(scope, &id, &shape, &loc)?;

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
      let shape = check_scope(scope, &id, &loc)?;

      Ok(Expression::Variable {shape, loc, id})
    }
    Expression::StringLiteral{..} => Ok(ex),
    Expression::NumberLiteral{..} => Ok(ex)
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
    Shape::BaseShape{..} => Ok(shape),
    _ => loc.fail("Unknown shape"),
  }
}

fn verify(defined: Shape, found: Shape, loc: &Location) -> Result<Shape, SimpleError> {
  if let Shape::UnknownShape = defined {
    Ok(found)
  } else {
    let filled_defined = fill_shape(defined, loc)?;

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
      Ok( (args.clone(), *result.clone()) )
    }
  } else {
    Err(SimpleError::new( format!("Function has type that is not a function type! Declared type: '{}' {}", defined.pretty(), loc.pretty())))
  }
}

fn pre_fill_module_function(scope: &mut Scope, ex: &Expression) -> Result<(), SimpleError> {
  if let Expression::FunctionDeclaration {shape, loc, id, ..} = ex {
    let shape = fill_shape(ex.shape().clone(), &ex.loc())?;

    set_scope(scope, id, &shape, &loc)
  } else {
    Err(SimpleError::new(format!("Invalid function declaration: {}", ex.loc().pretty())))
  }
}

fn set_scope(scope_stack: &mut Scope, id: &String, shape: &Shape, loc: &Location) -> Result<(), SimpleError> {
  let scope = scope_stack.last_mut().expect("Scope should never be empty!");

  if scope.contains_key(id) {
    Err(SimpleError::new(format!("Redeclaration of variable: {} {}", id, loc.pretty())))
  } else {
    scope.insert(id.clone(), shape.clone());
    Ok(())
  }
}

fn check_scope(scope_stack: &Scope, id: &String, loc: &Location) -> Result<Shape, SimpleError> {
  for i in (0..scope_stack.len()).rev() {
    let scope = &scope_stack[i];

    if scope.contains_key(id) {
      return Ok(scope[id].clone());
    }
  }

  Err(SimpleError::new(format!("Undeclared variable: {} {}", id, loc.pretty())))
}

fn create_scope(scope_stack: &mut Scope) {
  scope_stack.push(HashMap::new());
}

fn destroy_scope(scope_stack: &mut Scope) {
  scope_stack.pop();
}
