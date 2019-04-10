use std::cmp::max;
use std::collections::HashMap;
use std::rc::Rc;

use simple_error::SimpleError;

use ast::AssignmentEx;
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

pub struct Compiler {
  shape_refs: Vec<Shape>,
}

impl Compiler {

  pub fn compile_package(name: &str, base_dir: &str) -> Result<BitPackage, SimpleError> {
    let raw_modules = Compiler::find_modules(base_dir, name)?;

    let mut modules = HashMap::new();

    for parsed in raw_modules {
      let checked = typechecker::check_module(parsed)?;
      modules.insert(checked.name.clone(), Compiler::compile(checked)?);
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
    let me = Compiler {
      shape_refs: Vec::new(),
    };

    let core = CoreContext::new();

    me.compile_module(core, module)
  }

  fn compile_module(mut self, core: CoreContext, module: Module) -> Result<BitModule, SimpleError> {
    let mut context = ModuleContext::new(core, &module);

    for dec in &module.functions {
      context.add_function_ref(&dec.ex.id, dec.ex.shape());
    }

    for dec in &module.functions {
      let bit_func = self.compile_function(&mut context, &dec.ex)?;

      context.functions.insert(dec.ex.id.clone(), Rc::new(bit_func));
    }

    let Compiler { shape_refs } = self;
    let ModuleContext{string_constants, function_refs, functions, ..} = context;

    Ok(BitModule {
      string_constants,
      function_refs,
      functions,
      shape_refs,
    })
  }

  fn compile_function(&mut self, context: &mut ModuleContext, ex: &FunctionDeclarationEx) -> Result<BitFunction, SimpleError> {
    context.reset(ex.args.len() as LocalId);

    for closure in &ex.context.closures {
      context.store(closure);
    }

    for arg in &ex.args {
      context.store(&arg.id);
    }

    let mut body = self.compile_expression(context, &ex.body)?;

    // if we don't end with a return, add one anyway.
    if let Some(Instruction::Return) = body.last() {} else {
      body.push(Instruction::Return);
    }

    return Ok(BitFunction {
      package: context.package.clone(),
      module: context.module.clone(),

      max_locals: context.max_locals + 1,
      shape: ex.shape(),
      body,
      source: vec![],
    });
  }

  fn compile_expression(&mut self, context: &mut ModuleContext, ex: &Expression) -> Result<Vec<Instruction>, SimpleError> {
    match ex {
      Expression::FunctionDeclaration(ex) => ex.compile(self, context),
      Expression::Assignment(ex) => ex.compile(self, context),
      Expression::Variable(ex) => ex.compile(self, context),
      Expression::BinaryOp(ex) => ex.compile(self, context),
      Expression::Call(ex) => ex.compile(self, context),
      Expression::Block(ex) => ex.compile(self, context),
      Expression::StringLiteral(ex) => ex.compile(self, context),
      Expression::NumberLiteral(ex) => ex.compile(self, context),

      _ => unimplemented!()
    }
  }
}

trait Compilable {

  fn compile(&self, compiler: &mut Compiler, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError>;

}

impl Compilable for FunctionDeclarationEx {
  fn compile(&self, compiler: &mut Compiler, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {

    if self.context.closures.is_empty() {
      let full_id = format!("$closure:{}", context.gen_next_function_id());
      let bit_func = compiler.compile_function(context, self)?;
      let const_id = context.add_function_ref(&full_id, self.shape());
      context.functions.insert(full_id, Rc::new(bit_func));

      let mut body = vec![Instruction::LoadConst {kind: LoadType::Function, const_id}];

      if !self.context.is_lambda {
        let local = context.store(&self.id);
        body.push(Instruction::StoreValue { local });
      }

      return Ok(body);
    } else {
      let mut body = Vec::new();

      for local in &self.context.closures {
        let lookup = context.lookup(local, &self.loc)?;

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

      let full_id = format!("$closure:{}", context.gen_next_function_id());
      let bit_func = compiler.compile_function(context, self)?;
      let func_id = context.add_function_ref(&full_id, self.shape());
      context.functions.insert(full_id, Rc::new(bit_func));

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
  fn compile(&self, compiler: &mut Compiler, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {
    let AssignmentEx{ shape, loc, id, body } = self;
    let mut assign = compiler.compile_expression(context, body)?;
    let local = context.store(id);
    assign.push(Instruction::StoreValue { local });
    return Ok(assign);
  }
}

impl Compilable for VariableEx {
  fn compile(&self, compiler: &mut Compiler, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {
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
  fn compile(&self, compiler: &mut Compiler, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {
    let BinaryOpEx{ shape, loc, op, left, right } = self;
    let mut body = compiler.compile_expression(context, left)?;
    let mut other = compiler.compile_expression(context, right)?;
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
  fn compile(&self, compiler: &mut Compiler, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {
    let CallEx{ shape, loc, func, args } = self;
    let mut body = Vec::new();

    if let Expression::Variable(var) = func {
      if let Lookup::Static(func_id) = context.lookup(&var.id, loc)? {
        for arg in args {
          let mut more = compiler.compile_expression(context, arg)?;
          body.append(&mut more);
        }

        body.push(Instruction::CallStatic { func_id });
        return Ok(body);
      }
    }

    let mut function = compiler.compile_expression(context, func)?;
    body.append(&mut function);

    for arg in args {
      let mut more = compiler.compile_expression(context, arg)?;
      body.append(&mut more);
    }

    let func_shape = func.shape();

    let shape_id = compiler.shape_refs.iter().position(|other| *other == func_shape)
      .or_else(move || {
        compiler.shape_refs.push(func_shape.clone());
        Some(compiler.shape_refs.len() - 1)
      }).unwrap() as u32;

    body.push(Instruction::CallDynamic { shape_id });
    Ok(body)
  }
}

impl Compilable for BlockEx {
  fn compile(&self, compiler: &mut Compiler, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {
    let BlockEx{ shape, loc, body } = self;
    context.push_scope();
    let mut content: Vec<Instruction> = Vec::new();

    for next in body {
      let mut next_content = compiler.compile_expression(context, next)?;
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
  fn compile(&self, compiler: &mut Compiler, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {
    let StringLiteralEx{ shape, loc, value } = self;
    let const_id = context.string_constant(value);

    Ok(vec![Instruction::LoadConst { kind: LoadType::String, const_id }])
  }
}

impl Compilable for NumberLiteralEx {
  fn compile(&self, compiler: &mut Compiler, context: &mut ModuleContext) -> Result<Vec<Instruction>, SimpleError> {
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

  string_constant_map: HashMap<String, ConstantId>,
  string_constants: Vec<String>,

  max_locals: u16,
  local: Vec<FuncContext>,

  generated_function_count: usize,
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

      string_constant_map: HashMap::new(),
      string_constants: Vec::new(),

      max_locals: 0,
      local: vec![FuncContext::new(0)],

      generated_function_count: 0,
    }
  }

  fn reset(&mut self, max_locals: u16) {
    self.max_locals = max_locals;
    self.local = vec![FuncContext::new(max_locals)];
  }

  fn add_function_ref(&mut self, name: &str, shape: Shape) -> ConstantId {
    let ref_size = self.function_ref_map.len() as ConstantId;

    let func_ref = FunctionRef { name: String::from(name), shape: shape.clone() };
    self.function_ref_map.insert(String::from(name), (ref_size, func_ref));
    self.function_refs.push(FunctionRef { name: String::from(name), shape });

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
    for local in self.local.iter().rev() {
      if let Some(lookup) = local.lookup(name) {
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

  fn string_constant(&mut self, s: &str) -> ConstantId {
    if let Some(id) = self.string_constant_map.get(s) {
      return id.clone();
    }

    let id = self.string_constant_map.len() as ConstantId;
    self.string_constant_map.insert(s.to_string(), id);
    self.string_constants.push(s.to_string());
    id
  }

  fn store(&mut self, id: &str) -> u16 {
    self.local.last_mut().unwrap().store(id)
  }

  fn push_scope(&mut self) {
    self.local.push(FuncContext::new(self.max_locals));
  }

  fn pop_scope(&mut self) {
    if let Some(last) = self.local.pop() {
      self.max_locals = max(self.max_locals, last.max_locals);
    }
  }

  fn gen_next_function_id(&mut self) -> usize {
    let id = self.generated_function_count;
    self.generated_function_count = id + 1;
    id
  }
}

enum Lookup {
  Local(LocalId),
  Static(ConstantId),
}

struct FuncContext {
  max_locals: u16,
  locals: HashMap<String, u16>,
}

impl FuncContext {
  fn new(max_locals: u16) -> FuncContext {
    FuncContext {
      max_locals,
      locals: HashMap::new(),
    }
  }

  fn store(&mut self, id: &str) -> u16 {
    let local_id = self.locals.len() as u16;

    self.locals.insert(String::from(id), local_id);
    self.max_locals = max(self.max_locals, local_id);

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

