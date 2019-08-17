use std::collections::HashMap;

use simple_error::*;

use ast::*;
use shapes::*;
use ir::IrModule;

pub fn check_module(module: AstModule) -> Result<AstModule, SimpleError> {
  let mut app = AppShapes::new();
  let mut imports = module.imports.clone();
  let mut functions = Vec::new();

  let mut scope = Scope::new();
  scope.create_function_scope();

  for imp in &imports {
    let module_name = &imp.module.clone();
    let module = app.lookup_module(&imp.package, &imp.module)
      .ok_or_else(|| SimpleError::new("No such module"))?;

    for func in module.list_values() {
      let shape = module.lookup(&func).expect("Invalid impl");
      scope.pre_fill_module_function( format!("{}.{}", module_name, func), shape, &imp.loc);
    }
  }

  for dec in &module.functions {
    scope.pre_fill_module_function(dec.ex.id.clone(), dec.ex.shape(), &dec.ex.loc)?;
  }

  for dec in module.functions {
    if let Expression::FunctionDeclaration(content) = dec.ex.check(&mut scope, shape_unknown())? {
      functions.push(AstFunctionDeclaration {visibility: dec.visibility, ex: *content});
    } else {
      return Err(SimpleError::new("FunctionDeclaration didn't return itself!"))
    }
  }

  Ok(AstModule { package: module.package, name: module.name, functions, imports })
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

    let id = self.id.clone();
    let body = check(scope, self.body, self.result.clone())?;

    let returned_shape = body.shape();

    let result = verify(self.result, returned_shape, &self.loc)?;

    let closures = scope.destroy_function_scope();

    let before_size = closures.len();
    let maybe_me: Vec<Parameter> = closures.into_iter().filter(|param| param.id != id).collect();

    let context = if before_size != maybe_me.len() {
      self.context.set_is_recursive(true)
        .set_closures(maybe_me)
    } else {
      self.context.set_closures(maybe_me)
    };

    Ok(FunctionDeclarationEx{result, body, id, args, loc: self.loc, context}.wrap())
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

const FLOAT_OPS: &'static [&'static str] = &["+", "-", "*", "/"];
const COMPARE_OPS: &'static [&'static str] = &["==", "!=", "<", ">", "<=", ">="];

impl Typed for BinaryOpEx {
  fn check(self, scope: &mut Scope, expected: Shape) -> Result<Expression, SimpleError> {
    let BinaryOpEx{shape: raw_shape, left: raw_left, right: raw_right, op, loc} = self;

    let result_shape = if FLOAT_OPS.contains(&op.as_str()) {
      shape_float()
    } else {
      shape_boolean()
    };

    let left = check(scope, raw_left, shape_float())?;
    let right = check(scope, raw_right, shape_float())?;

    if left.shape() == right.shape() {
      Ok(BinaryOpEx{shape: result_shape, left, right, op, loc}.wrap())
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

impl Typed for IfEx {
  fn check(self, scope: &mut Scope, expected: Shape) -> Result<Expression, SimpleError> {
    let IfEx{shape: raw_shape, loc, condition: raw_condition, then_block: raw_then_block, else_block: raw_else_block} = self;

    let condition = check(scope, raw_condition, shape_boolean())?;

    verify(shape_boolean(), condition.shape(), &loc)?;

    let then_block = check(scope, raw_then_block, shape_unknown())?;
    let else_block = check(scope, raw_else_block, shape_unknown())?;

    verify(then_block.shape(), else_block.shape(), &loc)?;

    Ok(IfEx{
      shape: then_block.shape(),
      loc,

      condition,
      then_block,
      else_block
    }.wrap())
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
    Expression::NoOp(_) => Ok(ex),
    Expression::Import(_) => Ok(ex),
    Expression::FunctionDeclaration(ex) => ex.check(scope, expected),
    Expression::Block(ex) => ex.check(scope, expected),
    Expression::Assignment(ex) => ex.check(scope, expected),
    Expression::BinaryOp(ex) => ex.check(scope, expected),
    Expression::Call(ex) => ex.check(scope, expected),
    Expression::If(ex) => ex.check(scope, expected),
    Expression::Variable(ex) => ex.check(scope, expected),
    Expression::StringLiteral(ex) => ex.check(scope, expected),
    Expression::NumberLiteral(ex) => ex.check(scope, expected),
    Expression::BooleanLiteral(..) => Ok(ex),
  }
}

fn fill_shape(shape: Shape, loc: &Location) -> Result<Shape, SimpleError> {
  match shape {
    Shape::GenericShapeConstructor{base, args} => {
      Ok(Shape::GenericShapeConstructor {
        base: Box::new(fill_shape(*base, loc)?),
        args
      })
    }
    Shape::GenericShape{base, args} => {
      let mut filled_args = Vec::new();

      for arg in args {
        filled_args.push(fill_shape(arg, loc)?)
      }

      Ok(Shape::GenericShape {
        base: Box::new(fill_shape(*base, loc)?),
        args: filled_args
      })
    },
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
  closures: Vec<Vec<Parameter>>,
}

impl Scope {

  fn new() -> Scope {
    Scope{
      static_scope: HashMap::new(),
      block_stack: Vec::new(),
      closures: Vec::new(),
    }
  }

  fn pre_fill_module_function(&mut self, id: String, shape: Shape, loc: &Location) -> Result<(), SimpleError> {
    let shape = fill_shape(shape, &loc)?;

    self.static_scope.insert(id, shape);
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
            let shape = scope.get(id).unwrap();
            let param = Parameter {
              id: id.clone(),
              shape: shape.clone(),
            };

            self.closures.last_mut().expect("closures should never be empty!").push(param);
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

  fn destroy_function_scope(&mut self) -> Vec<Parameter> {
    self.block_stack.pop();
    self.closures.pop()
      .expect("closures should never be empty!")
  }
}

pub struct AppShapes {
  packages: HashMap<String, Box<PackageShapes>>,
}

impl AppShapes {

  pub fn new() -> AppShapes {
    let mut packages = HashMap::new();

    packages.insert(String::from("Core"), core_package());

    AppShapes {
      packages
    }
  }

  fn lookup_module(&self, package: &str, module: &str) -> Option<&Box<ModuleShapes>> {
    self.packages.get(package).and_then(|pack| pack.lookup_module(module))
  }

  fn lookup(&self, package: &str, module: &str, name: &str) -> Option<Shape> {
    self.packages.get(package).and_then(|pack| pack.lookup(module, name))
  }

}

trait PackageShapes {

  fn lookup_module(&self, module: &str) -> Option<&Box<ModuleShapes>>;

  fn lookup(&self, module: &str, name: &str) -> Option<Shape>;

}

struct PackageShapesBundle {
  modules: HashMap<String, Box<ModuleShapes>>,
}

impl PackageShapes for PackageShapesBundle {
  fn lookup_module(&self, module: &str) -> Option<&Box<ModuleShapes>> {
    self.modules.get(module)
  }

  fn lookup(&self, module: &str, name: &str) -> Option<Shape> {
    self.modules.get(module).and_then(|module| module.lookup(name))
  }
}

trait ModuleShapes {

  fn lookup(&self, name: &str) -> Option<Shape>;

  fn list_values(&self) -> Vec<String>;

}

struct CoreModuleShapes {
  functions: HashMap<String, Shape>
}

impl ModuleShapes for IrModule {
  fn lookup(&self, name: &str) -> Option<Shape> {
    self.functions.get(name).map(|func| func.shape.clone())
  }

  fn list_values(&self) -> Vec<String> {
    self.functions.keys().into_iter().map(|i| i.clone()).collect()
  }
}

impl ModuleShapes for CoreModuleShapes {
  fn lookup(&self, name: &str) -> Option<Shape> {
    self.functions.get(name).map(|shape| shape.clone())
  }
  fn list_values(&self) -> Vec<String> {
    self.functions.keys().into_iter().map(|i| i.clone()).collect()
  }
}

fn core_package() -> Box<PackageShapes> {
  let mut modules = HashMap::new();

  modules.insert(String::from("Core"), core_module());
  modules.insert(String::from("List"), list_module());

  Box::new(PackageShapesBundle {
    modules
  })
}

fn list_module() -> Box<ModuleShapes> {
  let mut functions = HashMap::new();

  let float_list = shape_list(shape_float());

  functions.insert(String::from("new"), Shape::SimpleFunctionShape {
    args: vec![],
    result: Box::new(float_list.clone())
  });

  functions.insert(String::from("append"), Shape::SimpleFunctionShape {
    args: vec![float_list.clone(), shape_float()],
    result: Box::new(float_list.clone())
  });

  let mapper_shape = Shape::SimpleFunctionShape {
    args: vec![shape_float()],
    result: Box::new(shape_float())
  };

  functions.insert(String::from("map"), Shape::SimpleFunctionShape {
    args: vec![float_list.clone(), mapper_shape],
    result: Box::new(float_list.clone())
  });

  let reducer_shape = Shape::SimpleFunctionShape {
    args: vec![shape_float(), shape_float()],
    result: Box::new(shape_float())
  };

  functions.insert(String::from("fold"), Shape::SimpleFunctionShape {
    args: vec![float_list.clone(), shape_float(), reducer_shape],
    result: Box::new(shape_float())
  });

  Box::new(CoreModuleShapes {
    functions
  })
}

fn core_module() -> Box<ModuleShapes> {
  let mut functions = HashMap::new();
  let float_math = Shape::SimpleFunctionShape {
    args: vec![shape_float(), shape_float()],
    result: Box::new(shape_float())
  };
  let float_compare = Shape::SimpleFunctionShape {
    args: vec![shape_float(), shape_float()],
    result: Box::new(shape_boolean())
  };

  functions.insert(String::from("+"), float_math.clone());
  functions.insert(String::from("-"), float_math.clone());
  functions.insert(String::from("*"), float_math.clone());
  functions.insert(String::from("/"), float_math.clone());

  functions.insert(String::from("=="), float_compare.clone());
  functions.insert(String::from("!="), float_compare.clone());
  functions.insert(String::from(">"), float_compare.clone());
  functions.insert(String::from(">="), float_compare.clone());
  functions.insert(String::from("<"), float_compare.clone());
  functions.insert(String::from("<="), float_compare.clone());

  Box::new(CoreModuleShapes {
    functions
  })
}
