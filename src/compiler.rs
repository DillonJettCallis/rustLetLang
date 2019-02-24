use std::cmp::max;
use std::collections::HashMap;
use std::rc::Rc;

use simple_error::SimpleError;

use ast::Expression;
use ast::Location;
use ast::Module;
use bytecode::AppDirectory;
use bytecode::BitFunction;
use bytecode::ConstantId;
use bytecode::FunctionRef;
use bytecode::Instruction;
use bytecode::LoadType;
use bytecode::LocalId;
use interpreter::RunFunction;
use shapes::Shape;
use shapes::shape_float;

pub struct Compiler {
  shape_refs: Vec<Shape>,
}

impl Compiler {
  pub fn compile(module: Module) -> Result<AppDirectory, SimpleError> {
    let me = Compiler {
      shape_refs: Vec::new(),
    };

    let core = CoreContext::new();

    me.compile_app(core, module)
  }

  fn compile_app(mut self, core: CoreContext, module: Module) -> Result<AppDirectory, SimpleError> {
    let mut functions: HashMap<String, Rc<RunFunction>> = HashMap::new();
    let mut context = ModuleContext::new(core);


    for export in &module.exports {
      if let Expression::FunctionDeclaration {ref id, ref shape, ..} = *export.content {
        context.add_function_ref(id, shape.clone());
      }
    }

    for func in &module.locals {
      if let Expression::FunctionDeclaration {ref id, ref shape, ..} = **func {
        context.add_function_ref(id, shape.clone());
      }
    }



    for export in module.exports {
      let (id, bit_func) = self.compile_function(&mut context, &export.content)?;

      functions.insert(id, Rc::new(bit_func));
    }

    for func in module.locals {
      let (id, bit_func) = self.compile_function(&mut context, &func)?;

      functions.insert(id, Rc::new(bit_func));
    }

    let Compiler { shape_refs } = self;
    Ok(AppDirectory {
      string_constants: context.string_constants.clone(),
      function_refs: context.function_refs.clone(),
      functions,
      shape_refs,
    })
  }

  fn compile_function(&mut self, context: &mut ModuleContext, ex: &Expression) -> Result<(String, BitFunction), SimpleError> {
    if let Expression::FunctionDeclaration { shape, loc, id, args, body } = ex {
      context.reset(args.len() as LocalId);

      for arg in args {
        context.store(arg);
      }

      let mut body = self.compile_expression(context, body)?;

      // if we don't end with a return, add one anyway.
      if let Some(Instruction::Return) = body.last() {} else {
        body.push(Instruction::Return);
      }

      return Ok((id.clone(), BitFunction {
        max_locals: context.max_locals + 1,
        shape: shape.clone(),
        body,
        source: vec![],
      }));
    }

    Err(SimpleError::new("Attempt to call compile_function with non-function expression"))
  }

  fn compile_expression(&mut self, context: &mut ModuleContext, ex: &Expression) -> Result<Vec<Instruction>, SimpleError> {
    match ex {
      Expression::Assignment { shape, loc, id, body } => {
        let mut assign = self.compile_expression(context, body)?;
        let local = context.store(id);
        assign.push(Instruction::StoreValue { local });
        return Ok(assign);
      }
      Expression::Variable { shape, loc, id } => {
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
      Expression::BinaryOp { shape, loc, op, left, right } => {
        let mut body = self.compile_expression(context, left)?;
        let mut other = self.compile_expression(context, right)?;
        body.append(&mut other);

        let id = format!("Core.{}", op);
        if let Lookup::Static(func_id) = context.lookup(&id, loc)? {
          body.push(Instruction::CallStatic { func_id });
          return Ok(body);
        } else {
          return Err(SimpleError::new(format!("Could not look up Core operator function {}", op)));
        }
      }
      Expression::Call { shape, loc, func, args } => {
        let mut body = Vec::new();

        if let Expression::Variable { ref id, .. } = **func {
          if let Lookup::Static(func_id) = context.lookup(id, loc)? {
            for arg in args {
              let mut more = self.compile_expression(context, arg)?;
              body.append(&mut more);
            }

            body.push(Instruction::CallStatic { func_id });
            return Ok(body);
          }
        }

        let mut function = self.compile_expression(context, func)?;
        body.append(&mut function);

        for arg in args {
          let mut more = self.compile_expression(context, arg)?;
          body.append(&mut more);
        }

        let func_shape = func.shape();

        let shape_id = self.shape_refs.iter().position(|other| other == func_shape)
          .or_else(move || {
            self.shape_refs.push(func_shape.clone());
            Some(self.shape_refs.len() - 1)
          }).unwrap() as u32;

        body.push(Instruction::CallDynamic { shape_id });
        Ok(body)
      }
      Expression::Block { shape, loc, body } => {
        context.push_scope();
        let mut content: Vec<Instruction> = Vec::new();

        for next in body {
          let mut next_content = self.compile_expression(context, next)?;
          content.append(&mut next_content);
        }

        if content.is_empty() {
          // If the block is empty, return a Null so there is something there.
          content.push(Instruction::LoadConstNull)
        }

        context.pop_scope();
        return Ok(content);
      }
      Expression::StringLiteral { shape, loc, value } => {
        let const_id = context.string_constant(value);

        return Ok(vec![Instruction::LoadConst { kind: LoadType::String, const_id }]);
      }
      Expression::NumberLiteral { shape, loc, value } => {
        return Ok(vec![Instruction::LoadConstFloat { value: value.clone() }]);
      }

      _ => unimplemented!()
    }
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

  function_ref_map: HashMap<String, (ConstantId, FunctionRef)>,
  function_refs: Vec<FunctionRef>,

  string_constant_map: HashMap<String, ConstantId>,
  string_constants: Vec<String>,

  max_locals: u16,
  local: Vec<FuncContext>,
}

impl ModuleContext {
  fn new(core: CoreContext) -> ModuleContext {
    ModuleContext {
      core,
      function_ref_map: HashMap::new(),
      function_refs: Vec::new(),

      string_constant_map: HashMap::new(),
      string_constants: Vec::new(),

      max_locals: 0,
      local: vec![FuncContext::new(0)],
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
      Some((Lookup::Local(local_id.clone())))
    } else {
      None
    }
  }
}

