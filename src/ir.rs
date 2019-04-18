use std::collections::HashMap;
use std::hash::Hash;
use std::io;
use std::io::{BufWriter, Error, Write, Read};

use bincode::{serialize_into, deserialize_from};
use serde::{Deserialize, Serialize};
use simple_error::SimpleError;

use ast::{AssignmentEx, BinaryOpEx, BlockEx, CallEx, Expression, FunctionDeclarationEx, IfEx, Location, AstModule, NumberLiteralEx, Parameter, StringLiteralEx, VariableEx};
use bytecode::{FunctionRef, LocalId};
use ir::ScopeLookup::Local;
use shapes::{Shape, shape_boolean, shape_float};

#[derive(Serialize, Deserialize)]
pub struct IrModule {
  pub package: String,
  pub name: String,
  pub functions: HashMap<String, IrFunction>,
}

impl IrModule {
  pub fn debug(&self) -> Result<(), SimpleError> {
    let mut writer = io::stderr();

    writer.write_all(format!("{}::{} \n", self.package, self.name).as_bytes())
      .map_err(|err| SimpleError::from(err))?;

    for func in self.functions.values() {
      func.pretty_print(&mut writer)?;
    }

    Ok(())
  }
}

#[derive(Serialize, Deserialize)]
pub struct IrFunction {
  pub func_ref: FunctionRef,
  pub args: Vec<Parameter>,
  pub body: Vec<Ir>,
  pub shape: Shape,
}

impl IrFunction {
  pub fn debug(&self) {
    let mut writer = io::stderr();

    self.pretty_print(&mut writer);
  }

  pub fn pretty_print<Writer: Write>(&self, writer: &mut Writer) -> Result<(), SimpleError> {
    let args: Vec<String> = self.args.iter().map(|param| param.pretty()).collect();

    writer.write_all(format!("  {}({}): {}\n", self.func_ref.name, args.join(", "), self.func_ref.result().pretty()).as_bytes())
      .map_err(|err| SimpleError::from(err))?;


    Ir::pretty_print(&self.body, "    ", writer)?;

    writer.write_all(b"\n")
      .map_err(|err| SimpleError::from(err))
  }
}

#[derive(Serialize, Deserialize)]
pub enum Ir {
  NoOp,
  // 0 is an error to hopefully crash early on invalid bytecode.
  Duplicate,
  Pop,
  Swap,
  LoadConstNull,
  LoadConstTrue,
  LoadConstFalse,
  LoadConstString {
    value: String,
  },
  LoadConstFunction {
    value: FunctionRef,
  },
  LoadConstFloat {
    value: f64
  },
  LoadValue {
    local: String,
  },
  StoreValue {
    local: String,
  },
  CallStatic {
    func: FunctionRef,
  },
  CallDynamic {
    param_count: LocalId,
  },
  BuildClosure {
    param_count: LocalId,
    func: FunctionRef,
  },
  BuildRecursiveFunction,
  Return,
  Branch {
    then_block: Vec<Ir>,
    else_block: Vec<Ir>,
  },
  Debug,
  Error,
  FreeLocal {
    local: String,
  }
}

impl Ir {
  pub fn pretty_print<Writer: Write>(block: &Vec<Ir>, indent: &str, writer: &mut Writer) -> Result<(), SimpleError> {
    for (index, next) in block.iter().enumerate() {
      writer.write_all(format!("{}{}: ", indent, index).as_bytes()).map_err(|err| SimpleError::from(err))?;

      match next {
        Ir::NoOp => writer.write_all(b"NoOp"),
        Ir::Duplicate => writer.write_all(b"Duplicate"),
        Ir::Pop => writer.write_all(b"Pop"),
        Ir::Swap => writer.write_all(b"Swap"),
        Ir::LoadConstNull => writer.write_all(b"LoadConstNull"),
        Ir::LoadConstTrue => writer.write_all(b"LoadConstTrue"),
        Ir::LoadConstFalse => writer.write_all(b"LoadConstFalse"),
        Ir::LoadConstString { value } => writer.write_all(format!("LoadConstString('{}')", value).as_bytes()),
        Ir::LoadConstFunction { value } => writer.write_all(format!("LoadConstFunction({})", value.pretty()).as_bytes()),
        Ir::LoadConstFloat { value } => writer.write_all(format!("LoadConstFloat({})", value).as_bytes()),
        Ir::LoadValue { local } => writer.write_all(format!("LoadValue({})", local).as_bytes()),
        Ir::StoreValue { local } => writer.write_all(format!("StoreValue({})", local).as_bytes()),
        Ir::CallStatic { func } => writer.write_all(format!("CallStatic({})", func.pretty()).as_bytes()),
        Ir::CallDynamic { param_count } => writer.write_all(format!("CallDynamic({})", param_count).as_bytes()),
        Ir::BuildClosure { param_count, func } => writer.write_all(format!("BuildClosure({}, '{}')", *param_count, func.pretty()).as_bytes()),
        Ir::BuildRecursiveFunction => writer.write_all(b"BuildRecursiveFunction"),
        Ir::Return => writer.write_all(b"Return"),
        Ir::Branch{then_block, else_block} => {
          let inner_indent = format!("{}    ", indent);
          writer.write_all(format!("Branch\n{}  then_block:\n", indent).as_bytes())
            .map_err(|err| SimpleError::from(err))?;
          Ir::pretty_print(then_block, &inner_indent, writer)?;
          writer.write_all(format!("{}  else_block:\n", indent).as_bytes())
            .map_err(|err| SimpleError::from(err))?;
          Ir::pretty_print(else_block, &inner_indent, writer)?;
          Ok(())
        },
        Ir::Debug => writer.write_all(b"Debug"),
        Ir::Error => writer.write_all(b"Error"),
        Ir::FreeLocal {local} => writer.write_all(format!("FreeLocal({})", local).as_bytes())
      }.map_err(|err| SimpleError::from(err))?;

      writer.write_all(b"\n").map_err(|err| SimpleError::from(err))?;
    }

    Ok(())
  }
}

pub fn compile_ir_module(module: &AstModule) -> Result<IrModule, SimpleError> {
  let mut context = IrModuleContext::new(module.package.clone(), module.name.clone());

  for func in &module.functions {
    let func_ref = FunctionRef {
      package: module.package.clone(),
      module: module.name.clone(),
      name: func.ex.id.clone(),

      shape: func.ex.shape().clone(),
    };
    context.declared_functions.insert(func.ex.id.clone(), ScopeLookup::Static(func_ref));
  }

  for func in &module.functions {
    compile_ir_function(&func.ex, &mut context)?;
  }

  Ok(IrModule {
    package: module.package.clone(),
    name: module.name.clone(),

    functions: context.functions,
  })
}

fn compile_ir_function(ex: &FunctionDeclarationEx, context: &mut IrModuleContext) -> Result<FunctionRef, SimpleError> {
  context.push_function();

  for closure in &ex.context.closures {
    context.store(closure.id.clone());
  }

  if ex.context.is_recursive {
    context.store(ex.id.clone());
  }

  for arg in &ex.args {
    context.store(arg.id.clone());
  }

  compile_ir_expression(&ex.body, context)?;

  context.append(Ir::Return);

  return Ok(context.pop_function(ex));
}

fn compile_ir_expression(ex: &Expression, context: &mut IrModuleContext) -> Result<(), SimpleError> {
  match ex {
    Expression::NoOp(_) => Ok(()),
    Expression::FunctionDeclaration(ex) => ex.compile_ir(context),
    Expression::Assignment(ex) => ex.compile_ir(context),
    Expression::Variable(ex) => ex.compile_ir(context),
    Expression::BinaryOp(ex) => ex.compile_ir(context),
    Expression::Call(ex) => ex.compile_ir(context),
    Expression::If(ex) => ex.compile_ir(context),
    Expression::Block(ex) => ex.compile_ir(context),
    Expression::StringLiteral(ex) => ex.compile_ir(context),
    Expression::NumberLiteral(ex) => ex.compile_ir(context),
    Expression::BooleanLiteral(_, value) => {
      if *value {
        context.append(Ir::LoadConstTrue)
      } else {
        context.append(Ir::LoadConstFalse)
      }
      Ok(())
    }

    _ => unimplemented!()
  }
}

trait IrCompilable {
  fn compile_ir(&self, context: &mut IrModuleContext) -> Result<(), SimpleError>;
}

impl IrCompilable for StringLiteralEx {
  fn compile_ir(&self, context: &mut IrModuleContext) -> Result<(), SimpleError> {
    Ok(context.append(Ir::LoadConstString { value: self.value.clone() }))
  }
}

impl IrCompilable for NumberLiteralEx {
  fn compile_ir(&self, context: &mut IrModuleContext) -> Result<(), SimpleError> {
    Ok(context.append(Ir::LoadConstFloat { value: self.value }))
  }
}

impl IrCompilable for BlockEx {
  fn compile_ir(&self, context: &mut IrModuleContext) -> Result<(), SimpleError> {
    for ex in &self.body {
      compile_ir_expression(ex, context)?;
    }
    Ok(())
  }
}

impl IrCompilable for CallEx {
  fn compile_ir(&self, context: &mut IrModuleContext) -> Result<(), SimpleError> {
    let CallEx { shape, loc, func, args } = self;

    if let Expression::Variable(var) = func {
      if let ScopeLookup::Static(func_ref) = context.lookup(&var.id, loc)? {
        for arg in args {
          compile_ir_expression(arg, context)?;
        }

        context.append(Ir::CallStatic { func: func_ref });
        return Ok(());
      }
    }

    compile_ir_expression(func, context)?;

    for arg in args {
      compile_ir_expression(arg, context)?;
    }

    if let Shape::SimpleFunctionShape {args, ..} = func.shape() {
      context.append(Ir::CallDynamic { param_count: args.len() as LocalId });
    } else {
      return self.loc.fail("Function does not have function shape");
    }

    Ok(())
  }
}

impl IrCompilable for IfEx {
  fn compile_ir(&self, context: &mut IrModuleContext) -> Result<(), SimpleError> {
    let IfEx{shape: raw_shape, loc, condition, then_block: raw_then_block, else_block: raw_else_block} = self;

    compile_ir_expression(condition, context)?;

    context.push_block();
    compile_ir_expression(raw_then_block, context)?;
    let then_block = context.pop_block();

    context.push_block();
    compile_ir_expression(raw_else_block, context)?;
    let else_block = context.pop_block();

    context.append(Ir::Branch {then_block, else_block});
    Ok(())
  }
}

impl IrCompilable for BinaryOpEx {
  fn compile_ir(&self, context: &mut IrModuleContext) -> Result<(), SimpleError> {
    let BinaryOpEx { shape, loc, op, left, right } = self;
    compile_ir_expression(left, context)?;
    compile_ir_expression(right, context)?;

    if let ScopeLookup::Static(func) = context.lookup(&op, loc)? {
      context.append(Ir::CallStatic { func });
      Ok(())
    } else {
      loc.fail(&format!("Could not look up Core operator function {}", op))
    }
  }
}

impl IrCompilable for VariableEx {
  fn compile_ir(&self, context: &mut IrModuleContext) -> Result<(), SimpleError> {
    Ok(context.append(Ir::LoadValue { local: self.id.clone() }))
  }
}

impl IrCompilable for AssignmentEx {
  fn compile_ir(&self, context: &mut IrModuleContext) -> Result<(), SimpleError> {
    compile_ir_expression(&self.body, context)?;
    context.store(self.id.clone());
    Ok(context.append(Ir::StoreValue { local: self.id.clone() }))
  }
}

impl IrCompilable for FunctionDeclarationEx {
  fn compile_ir(&self, context: &mut IrModuleContext) -> Result<(), SimpleError> {
    if self.context.closures.is_empty() {
      let func_ref = compile_ir_function(self, context)?;

      context.append(Ir::LoadConstFunction { value: func_ref });

      if self.context.is_recursive {
        context.append(Ir::BuildRecursiveFunction);
      }

      if !self.context.is_lambda {
        context.store(self.id.clone());
        context.append((Ir::StoreValue { local: self.id.clone() }));
      }

      return Ok(());
    } else {
      for local in &self.context.closures {
        let lookup = context.lookup(&local.id, &self.loc)?;

        match lookup {
          ScopeLookup::Local => {
            context.append(Ir::LoadValue { local: local.id.clone() })
          }
          ScopeLookup::Static(value) => {
            context.append(Ir::LoadConstFunction { value })
          }
        }
      }

      let func = compile_ir_function(self, context)?;

      context.append(Ir::BuildClosure { param_count: self.context.closures.len() as LocalId, func });

      if self.context.is_recursive {
        context.append(Ir::BuildRecursiveFunction);
      }

      if !self.context.is_lambda {
        context.store(self.id.clone());
        context.append(Ir::StoreValue { local: self.id.clone() });
      }

      return Ok(());
    }
  }
}

struct IrCoreContext {
  scope: HashMap<String, ScopeLookup>,
}

impl IrCoreContext {
  fn new() -> IrCoreContext {
    let mut scope = HashMap::new();

    let float_op = Shape::SimpleFunctionShape {
      args: vec![shape_float(), shape_float()],
      result: Box::new(shape_float()),
    };

    let float_compare_op = Shape::SimpleFunctionShape {
      args: vec![shape_float(), shape_float()],
      result: Box::new(shape_boolean()),
    };

    fn insert(scope: &mut HashMap<String, ScopeLookup>, name: &'static str, shape: Shape) {
      scope.insert(String::from(name), ScopeLookup::Static(FunctionRef {
        package: String::from("Core"),
        module: String::from("Core"),
        name: String::from(name),
        shape,
      }));
    };

    insert(&mut scope, "+", float_op.clone());
    insert(&mut scope, "-", float_op.clone());
    insert(&mut scope, "*", float_op.clone());
    insert(&mut scope, "/", float_op.clone());

    insert(&mut scope, "==", float_compare_op.clone());
    insert(&mut scope, "!=", float_compare_op.clone());
    insert(&mut scope, ">", float_compare_op.clone());
    insert(&mut scope, "<", float_compare_op.clone());
    insert(&mut scope, ">=", float_compare_op.clone());
    insert(&mut scope, "<=", float_compare_op.clone());

    IrCoreContext {
      scope
    }
  }
}

struct IrModuleContext {
  core: IrCoreContext,
  package: String,
  module: String,

  declared_functions: HashMap<String, ScopeLookup>,
  functions: HashMap<String, IrFunction>,

  function_context: Vec<IrFuncContext>,
}

impl IrModuleContext {
  fn new(package: String, module: String) -> IrModuleContext {
    IrModuleContext {
      core: IrCoreContext::new(),
      package,
      module,

      declared_functions: HashMap::new(),
      functions: HashMap::new(),
      function_context: Vec::new(),
    }
  }

  fn append(&mut self, ir: Ir) {
    self.function_context.last_mut().unwrap().append(ir)
  }

  fn lookup(&self, name: &str, loc: &Location) -> Result<ScopeLookup, SimpleError> {
    for func in self.function_context.iter().rev() {
      if let Some(lookup) = func.lookup(name) {
        return Ok(lookup);
      }
    }

    if let Some(func) = self.declared_functions.get(name) {
      return Ok(func.clone());
    }

    if let Some(core) = self.core.scope.get(name) {
      return Ok(core.clone());
    }

    loc.fail(&format!("Variable '{}' not found in IrCompiler scope", name))
  }

  fn store(&mut self, name: String) {
    self.function_context.last_mut().unwrap().store(name);
  }

  fn push_function(&mut self) {
    self.function_context.push(IrFuncContext::new())
  }

  fn pop_function(&mut self, ex: &FunctionDeclarationEx) -> FunctionRef {
    let mut context = self.function_context.pop().unwrap();

    let func_ref = FunctionRef {
      package: self.package.clone(),
      module: self.module.clone(),
      name: ex.id.clone(),

      shape: ex.shape().clone(),
    };

    let mut args = ex.context.closures.clone();

    if ex.context.is_recursive {
      args.push(Parameter{id: ex.id.clone(), shape: ex.shape().clone() });
    }

    args.append(&mut ex.args.clone());

    let func = IrFunction {
      func_ref: func_ref.clone(),
      args,
      body: context.pop_block(),
      shape: ex.shape().clone(),
    };

    self.functions.insert(ex.id.clone(), func);
    func_ref
  }

  fn push_block(&mut self) {
    self.function_context.last_mut().unwrap().push_block()
  }

  fn pop_block(&mut self) -> Vec<Ir> {
    self.function_context.last_mut().unwrap().pop_block()
  }
}

struct IrFuncContext {
  pub body: Vec<Vec<Ir>>,

  scope_stack: Vec<IrScope>,
}

impl IrFuncContext {
  fn new() -> IrFuncContext {
    IrFuncContext {
      body: vec![Vec::new()],

      scope_stack: vec![IrScope::new()],
    }
  }

  fn append(&mut self, ir: Ir) {
    self.body.last_mut().unwrap().push(ir)
  }

  fn lookup(&self, name: &str) -> Option<ScopeLookup> {
    for scope in self.scope_stack.iter().rev() {
      if let Some(lookup) = scope.scope.get(name) {
        return Some(lookup.clone());
      }
    }

    None
  }

  fn store(&mut self, name: String) {
    self.scope_stack.last_mut().unwrap().scope.insert(name, ScopeLookup::Local);
  }

  fn push_block(&mut self) {
    self.body.push(Vec::new())
  }

  fn pop_block(&mut self) -> Vec<Ir> {
    self.body.pop().unwrap()
  }
}

#[derive(Clone)]
enum ScopeLookup {
  Static(FunctionRef),
  Local,
}

struct IrScope {
  scope: HashMap<String, ScopeLookup>,
}

impl IrScope {
  fn new() -> IrScope {
    IrScope {
      scope: HashMap::new()
    }
  }
}


pub fn serialize_ir_module<Writer: Write>(writer: &mut Writer, module: &IrModule) -> Result<(), SimpleError> {
  serialize_into(writer, module)
    .map_err(|err| SimpleError::from(err))
}

pub fn deserialize_ir_module<Reader: Read>(reader: &mut Reader) -> Result<IrModule, SimpleError> {
  deserialize_from(reader)
    .map_err(|err| SimpleError::from(err))
}
