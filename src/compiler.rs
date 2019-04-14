use std::cmp::max;
use std::collections::HashMap;
use std::rc::Rc;

use simple_error::SimpleError;

use ast::{AssignmentEx, FunctionContext};
use ast::BinaryOpEx;
use ast::BlockEx;
use ast::CallEx;
use ast::Expression;
use ast::FunctionDeclarationEx;
use ast::Location;
use ast::Module;
use ast::NumberLiteralEx;
use ast::StringLiteralEx;
use ast::VariableEx;
use bytecode::{BitModule, BitPackage};
use bytecode::BitFunction;
use bytecode::ConstantId;
use bytecode::FunctionRef;
use bytecode::Instruction;
use bytecode::LoadType;
use bytecode::LocalId;
use interpreter::RunFunction;
use shapes::Shape;
use shapes::shape_float;

use std::fs::{self, DirEntry};
use std::path::{Path, PathBuf};
use parser::parse;
use typechecker;
use optimize::Optimizer;
use core::borrow::BorrowMut;

pub fn compile_package(name: &str, base_dir: &str) -> Result<BitPackage, SimpleError> {
  let raw_modules = find_modules(base_dir, name)?;

  let mut modules = HashMap::new();

  for parsed in raw_modules {
    let checked = typechecker::check_module(parsed)?;
    modules.insert(checked.name.clone(), compile(checked)?);
  }

  Ok(BitPackage {
    modules
  })
}

fn find_modules(base: &str, package: &str) -> Result<Vec<Module>, SimpleError> {
  let mut modules = Vec::new();
  let mut dirs = vec![Path::new(base).to_path_buf()];

  while !dirs.is_empty() {
    let next_dir = dirs.pop().unwrap();

    for entry in fs::read_dir(next_dir).map_err(|err| SimpleError::from(err))? {
      let entry = entry.map_err(|err| SimpleError::from(err))?;
      let path = entry.path();
      if path.is_dir() {
        dirs.push(path.clone())
      } else if path.extension().and_then(|ex| ex.to_str()).filter(|ex| *ex == "let").is_some() {
        let full_module = path.strip_prefix(base).map_err(|err| SimpleError::from(err))?
          .to_str()
          .ok_or_else(|| SimpleError::new("Invalid path"))?
          .replace("/", ".") // handle both *nix and windows paths
          .replace("\\", ".");

        // remove .let at the end
        let module = &full_module[..full_module.len() - 4];

        let parsed = parse(&path, package, module)?;
        modules.push(parsed);
      }
    }
  }

  Ok(modules)
}

pub fn compile(module: Module) -> Result<BitModule, SimpleError> {
  let core = CoreContext::new();

  compile_module(core, module)
}

fn compile_module(core: CoreContext, module: Module) -> Result<BitModule, SimpleError> {
  let mut module_context = ModuleContext::new(core, &module);

  for dec in &module.functions {
    module_context.add_function_ref(&dec.ex.id, dec.ex.shape());
  }

  let mut raw_funcs = HashMap::new();

  for dec in &module.functions {
    let bit_func = compile_function(&mut module_context, &dec.ex)?;

    raw_funcs.insert(dec.ex.id.clone(), bit_func);
  }

  let ModuleContext { 
    string_constants, 
    function_refs, 
    shape_refs, 
    functions, 
    .. 
  } = module_context;

  let mut raw_module = BitModule {
    string_constants,
    function_refs,
    functions,
    shape_refs,
  };

  let opts = Optimizer::new();

  for (name, mut func) in raw_funcs {
    opts.optimize(&mut raw_module, &mut func);
    raw_module.functions.insert(name, Rc::new(func));
  }

  Ok(raw_module)
}

fn compile_function(context: &mut ModuleContext, ex: &FunctionDeclarationEx) -> Result<BitFunction, SimpleError> {
  let package = context.package.clone();
  let module = context.module.clone();

  context.push_function();

  for closure in &ex.context.closures {
    context.store(&closure.id);
  }

  for arg in &ex.args {
    context.store(&arg.id);
  }

  let mut body = compile_expression(context, &ex.body)?;

  // if we don't end with a return, add one anyway.
  if let Some(Instruction::Return) = body.last() {} else {
    body.push(Instruction::Return);
  }

  let func_context = context.pop_function();

  return Ok(BitFunction {
    package,
    module,
    name: ex.id.clone(),

    max_locals: func_context.max_locals + 1,
    shape: ex.shape(),
    body,
    source: vec![],
  });
}

fn compile_expression(context: &mut ModuleContext, ex: &Expression) -> Result<Vec<Instruction>, SimpleError> {
  match ex {
    Expression::FunctionDeclaration(ex) => ex.compile( context),
    Expression::Assignment(ex) => ex.compile( context),
    Expression::Variable(ex) => ex.compile( context),
    Expression::BinaryOp(ex) => ex.compile( context),
    Expression::Call(ex) => ex.compile( context),
    Expression::Block(ex) => ex.compile( context),
    Expression::StringLiteral(ex) => ex.compile( context),
    Expression::NumberLiteral(ex) => ex.compile( context),

    _ => unimplemented!()
  }
}


trait Compilable {

  fn compile(&self, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError>;

}

impl Compilable for FunctionDeclarationEx {
  fn compile(&self, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {

    if self.context.closures.is_empty() {
      let full_id = self.id.clone();
      let bit_func = compile_function( context, self)?;
      let const_id = context.add_function_ref(&full_id, self.shape());
      context.add_function(full_id, bit_func);

      let mut body = vec![Instruction::LoadConst {kind: LoadType::Function, const_id}];

      if !self.context.is_lambda {
        let local = context.store(&self.id);
        body.push(Instruction::StoreValue { local });
      }

      return Ok(body);
    } else {
      let mut body = Vec::new();

      for local in &self.context.closures {
        let lookup = context.lookup(&local.id, &self.loc)?;

        match lookup {
          Lookup::Local(local) => {
            body.push(Instruction::LoadValue { local })
          }
          Lookup::Static(const_id) => {
            body.push(Instruction::LoadConst {
              kind: LoadType::Function,
              const_id,
            })
          }
        }
      }

      let full_id = self.id.clone();
      let bit_func = compile_function(context, self)?;
      let func_id = context.add_function_ref(&full_id, self.shape());
      context.add_function(full_id, bit_func);

      body.push(Instruction::BuildClosure {param_count: self.context.closures.len() as LocalId, func_id});

      if !self.context.is_lambda {
        let local = context.store(&self.id);
        body.push(Instruction::StoreValue { local });
      }

      return Ok(body);
    }
  }
}

impl Compilable for AssignmentEx {
  fn compile(&self, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {
    let AssignmentEx{ shape, loc, id, body } = self;
    let mut assign = compile_expression(context, body)?;
    let local = context.store(id);
    assign.push(Instruction::StoreValue { local });
    return Ok(assign);
  }
}

impl Compilable for VariableEx {
  fn compile(&self, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {
    let VariableEx{ shape, loc, id } = self;
    let lookup = context.lookup(id, loc)?;

    match lookup {
      Lookup::Local(local) => {
        Ok(vec![Instruction::LoadValue { local }])
      }
      Lookup::Static(const_id) => {
        Ok(vec![Instruction::LoadConst {
          kind: LoadType::Function,
          const_id,
        }])
      }
    }
  }
}

impl Compilable for BinaryOpEx {
  fn compile(&self, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {
    let BinaryOpEx{ shape, loc, op, left, right } = self;
    let mut body = compile_expression(context, left)?;
    let mut other = compile_expression(context, right)?;
    body.append(&mut other);

    let id = format!("Core.{}", op);
    if let Lookup::Static(func_id) = context.lookup(&id, loc)? {
      body.push(Instruction::CallStatic { func_id });
      return Ok(body);
    } else {
      return Err(SimpleError::new(format!("Could not look up Core operator function {}", op)));
    }
  }
}

impl Compilable for CallEx {
  fn compile(&self, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {
    let CallEx{ shape, loc, func, args } = self;
    let mut body = Vec::new();

    if let Expression::Variable(var) = func {
      if let Lookup::Static(func_id) = context.lookup(&var.id, loc)? {
        for arg in args {
          let mut more = compile_expression(context, arg)?;
          body.append(&mut more);
        }

        body.push(Instruction::CallStatic { func_id });
        return Ok(body);
      }
    }

    let mut function = compile_expression(context, func)?;
    body.append(&mut function);

    for arg in args {
      let mut more = compile_expression(context, arg)?;
      body.append(&mut more);
    }

    let shape_id = context.lookup_shape(func.shape());

    body.push(Instruction::CallDynamic { shape_id });
    Ok(body)
  }
}

impl Compilable for BlockEx {
  fn compile(&self, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {
    let BlockEx{ shape, loc, body } = self;
    context.push_scope();
    let mut content: Vec<Instruction> = Vec::new();

    for next in body {
      let mut next_content = compile_expression(context, next)?;
      content.append(&mut next_content);
    }

    if content.is_empty() {
      // If the block is empty, return a Null so there is something there.
      content.push(Instruction::LoadConstNull)
    }

    context.pop_scope();
    Ok(content)
  }
}

impl Compilable for StringLiteralEx {
  fn compile(&self, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {
    let StringLiteralEx{ shape, loc, value } = self;
    let const_id = context.lookup_string_constant(value);

    Ok(vec![Instruction::LoadConst { kind: LoadType::String, const_id }])
  }
}

impl Compilable for NumberLiteralEx {
  fn compile(&self, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {
    Ok(vec![Instruction::LoadConstFloat { value: self.value.clone() }])
  }
}

struct CoreContext {
  function_map: HashMap<String, Shape>,
}

impl CoreContext {
  fn new() -> CoreContext {
    let mut function_map = HashMap::new();
    let float_op = Shape::SimpleFunctionShape {
      args: vec![shape_float(), shape_float()],
      result: Box::new(shape_float()),
    };

    function_map.insert(String::from("Core.+"), float_op.clone());
    function_map.insert(String::from("Core.-"), float_op.clone());
    function_map.insert(String::from("Core.*"), float_op.clone());
    function_map.insert(String::from("Core./"), float_op.clone());

    return CoreContext {
      function_map
    };
  }
}

struct ModuleContext {
  core: CoreContext,
  package: String,
  module: String,

  functions: HashMap<String, Rc<RunFunction>>,

  function_ref_map: HashMap<String, (ConstantId, FunctionRef)>,
  function_refs: Vec<FunctionRef>,

  shape_refs: Vec<Shape>,

  string_constant_map: HashMap<String, ConstantId>,
  string_constants: Vec<String>,

  function_context: Vec<FuncContext>,
}

impl ModuleContext {
  fn new(core: CoreContext, module: &Module) -> ModuleContext {
    ModuleContext {
      core,
      package: module.package.clone(),
      module: module.name.clone(),
      functions: HashMap::new(),

      function_ref_map: HashMap::new(),
      function_refs: Vec::new(),

      shape_refs: Vec::new(),

      string_constant_map: HashMap::new(),
      string_constants: Vec::new(),

      function_context: Vec::new(),
    }
  }

  fn add_function_ref(&mut self, name: &str, shape: Shape) -> ConstantId {
    if let Some((id, _)) = self.function_ref_map.get(name) {
      return id.clone();
    }

    let ref_size = self.function_ref_map.len() as ConstantId;

    let func_ref = FunctionRef { package: self.package.clone(), module: self.module.clone(), name: String::from(name), shape: shape.clone() };
    self.function_ref_map.insert(String::from(name), (ref_size, func_ref.clone()));
    self.function_refs.push(func_ref);

    ref_size
  }

  fn lookup_core(&self, name: &str) -> Option<Shape> {
    if let Some(shape) = self.core.function_map.get(name) {
      Some(shape.clone())
    } else {
      None
    }
  }

  fn lookup(&mut self, name: &str, loc: &Location) -> Result<Lookup, SimpleError> {
    for func in self.function_context.iter().rev() {
      if let Some(lookup) = func.lookup(name) {
        return Ok(lookup);
      }
    }

    if let Some((func_id, _)) = self.function_ref_map.get(name) {
      return Ok(Lookup::Static(func_id.clone()));
    }

    if let Some(shape) = self.lookup_core(name) {
      let ref_size = self.add_function_ref(name, shape);
      return Ok(Lookup::Static(ref_size));
    } else {
      return loc.fail(&format!("Variable '{}' not found in compiler context", name));
    }
  }

  fn lookup_string_constant(&mut self, s: &str) -> ConstantId {
    if let Some(id) = self.string_constant_map.get(s) {
      return id.clone();
    }

    let id = self.string_constant_map.len() as ConstantId;
    self.string_constant_map.insert(s.to_string(), id);
    self.string_constants.push(s.to_string());
    id
  }

  fn lookup_shape(&mut self, shape: Shape) -> ConstantId {
    self.shape_refs.iter().position(|other| *other == shape)
      .or_else(move || {
        self.shape_refs.push(shape);
        Some(self.shape_refs.len() - 1)
      }).unwrap() as ConstantId
  }

  fn add_function(&mut self, id: String, func: BitFunction) {
    self.functions.insert(id,  Rc::new(func));
  }

  fn push_function(&mut self) {
    self.function_context.push(FuncContext::new());
  }

  fn pop_function(&mut self) -> FuncContext {
    self.function_context.pop().unwrap()
  }

  fn store(&mut self, id: &str) -> u16 {
    self.function_context.last_mut().unwrap().store(id)
  }

  fn push_scope(&mut self) {
    self.function_context.last_mut().unwrap().push_scope()
  }

  fn pop_scope(&mut self) {
    self.function_context.last_mut().unwrap().pop_scope()
  }
}

enum Lookup {
  Local(LocalId),
  Static(ConstantId),
}

struct FuncContext<> {
  max_locals: u16,
  local: Vec<BlockContext>,
}

impl FuncContext {

  fn new() -> FuncContext {
    FuncContext {
      max_locals: 0,
      local: vec![BlockContext::new(0)],
    }
  }

  fn lookup(&self, name: &str) -> Option<Lookup> {
    for local in self.local.iter().rev() {
      if let Some(lookup) = local.lookup(name) {
        return Some(lookup);
      }
    }

    None
  }

  fn store(&mut self, id: &str) -> u16 {
    self.local.last_mut().unwrap().store(id)
  }

  fn push_scope(&mut self) {
    let last_local_index = self.local.last().unwrap().max_locals;
    self.local.push(BlockContext::new(last_local_index));
  }

  fn pop_scope(&mut self) {
    if let Some(last) = self.local.pop() {
      self.max_locals = max(self.max_locals, last.max_locals);
    }
  }
}

struct BlockContext {
  max_locals: u16,
  locals: HashMap<String, u16>,
}

impl BlockContext {
  fn new(max_locals: u16) -> BlockContext {
    BlockContext {
      max_locals,
      locals: HashMap::new(),
    }
  }

  fn store(&mut self, id: &str) -> u16 {
    let local_id = self.max_locals;
    self.max_locals += 1;

    self.locals.insert(String::from(id), local_id);

    local_id
  }

  fn lookup(&self, id: &str) -> Option<Lookup> {
    if let Some(local_id) = self.locals.get(id) {
      Some(Lookup::Local(local_id.clone()))
    } else {
      None
    }
  }
}

