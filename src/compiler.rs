use core::borrow::BorrowMut;
use std::cmp::max;
use std::collections::HashMap;
use std::fs::{self, DirEntry, File, create_dir_all};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use simple_error::SimpleError;

use ast::{AssignmentEx, FunctionContext, Parameter};
use ast::BinaryOpEx;
use ast::BlockEx;
use ast::CallEx;
use ast::Expression;
use ast::FunctionDeclarationEx;
use ast::Location;
use ast::AstModule;
use ast::NumberLiteralEx;
use ast::StringLiteralEx;
use ast::VariableEx;
use bytecode::{BitModule, BitPackage};
use bytecode::BitFunction;
use bytecode::ConstantId;
use bytecode::FunctionRef;
use bytecode::Instruction;
use bytecode::LocalId;
use interpreter::RunFunction;
use ir::{compile_ir_module, Ir, IrFunction, IrModule};
use optimize::Optimizer;
use parser::parse;
use shapes::Shape;
use shapes::shape_float;
use typechecker;

pub fn compile_package(name: &str, base_dir: &str) -> Result<BitPackage, SimpleError> {
  let raw_modules = find_modules(base_dir, name)?;

  let mut modules = HashMap::new();

  for parsed in raw_modules {
    let checked = typechecker::check_module(parsed)?;
    let compiled = compile_ir_module(&checked)?;
    let bytecode = compile(compiled)?;
    bytecode.debug();
    modules.insert(checked.name.clone(), bytecode);
  }

  Ok(BitPackage {
    modules
  })
}

fn find_modules(base: &str, package: &str) -> Result<Vec<AstModule>, SimpleError> {
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

pub fn compile(mut module: IrModule) -> Result<BitModule, SimpleError> {
  let mut context = ModuleContext::new();
  let optimizer = Optimizer::new();
  let mut functions = HashMap::<String, RunFunction>::new();

  for (name, mut raw_func) in module.functions {
    optimizer.optimize(&mut raw_func);
//    raw_func.debug();

    let mut func_context = FuncContext::new(&raw_func.args);

    let body = compile_block(&mut context, &mut func_context, &raw_func.body);

    functions.insert(name.clone(), BitFunction {
      func_ref: FunctionRef {
        package: module.package.clone(),
        module: module.name.clone(),
        name: name.clone(),

        shape: raw_func.shape.clone(),
      },

      max_locals: func_context.max_locals,
      body,
      source: Vec::new(),
    }.wrap());
  }

  let ModuleContext{function_refs, shape_refs, string_constants} = context;

  Ok(BitModule {
    string_constants,
    function_refs,
    shape_refs,
    functions,
  })
}

fn compile_block(context: &mut ModuleContext, func: &mut FuncContext, block: &Vec<Ir>) -> Vec<Instruction> {
  let mut body = Vec::new();

  for next in block {
    match next {
      Ir::NoOp => body.push(Instruction::NoOp),
      Ir::Duplicate => body.push(Instruction::Duplicate),
      Ir::Pop => body.push(Instruction::Pop),
      Ir::Swap => body.push(Instruction::Swap),
      Ir::LoadConstNull => body.push(Instruction::LoadConstNull),
      Ir::LoadConstTrue => body.push(Instruction::LoadConstTrue),
      Ir::LoadConstFalse => body.push(Instruction::LoadConstFalse),
      Ir::LoadConstString { value } => body.push(Instruction::LoadConstString{const_id: context.lookup_string_constant(value)}),
      Ir::LoadConstFunction { value } => body.push(Instruction::LoadConstFunction{const_id: context.lookup_function_ref(value)}),
      Ir::LoadConstFloat { value } => body.push(Instruction::LoadConstFloat {value: *value}),
      Ir::LoadValue { local } => body.push(Instruction::LoadValue {local: func.lookup_local(local)}),
      Ir::StoreValue { local } => body.push(Instruction::StoreValue {local: func.lookup_local(local)}),
      Ir::CallStatic { func } => body.push(Instruction::CallStatic {func_id: context.lookup_function_ref(func) }),
      Ir::CallDynamic { param_count } => body.push(Instruction::CallDynamic {param_count: *param_count}),
      Ir::BuildClosure { param_count, func } => body.push(Instruction::BuildClosure {param_count: *param_count, func_id: context.lookup_function_ref(func) }),
      Ir::BuildRecursiveFunction => body.push(Instruction::BuildRecursiveFunction),
      Ir::Return => body.push(Instruction::Return),
      Ir::Branch{then_block, else_block} => {

        let mut then_body = compile_block(context, func, then_block);
        let mut else_body = compile_block(context, func, else_block);

        if !else_body.is_empty() {
          if let Some(Instruction::Return) = then_body.last() {

          } else {
            then_body.push(Instruction::Jump { jump: else_body.len() as i32 });
          }
        }

        body.push(Instruction::Branch {jump: then_body.len() as i32});
        body.append(&mut then_body);
        body.append(&mut else_body);
      },
      Ir::Debug => body.push(Instruction::Debug),
      Ir::Error => body.push(Instruction::Error),
      Ir::FreeLocal {local} => func.free(local),
    }
  }

  body
}

struct ModuleContext {
  function_refs: Vec<FunctionRef>,
  shape_refs: Vec<Shape>,
  string_constants: Vec<String>,
}

impl ModuleContext {
  fn new() -> ModuleContext {
    ModuleContext {
      function_refs: Vec::new(),
      shape_refs: Vec::new(),
      string_constants: Vec::new(),
    }
  }

  fn lookup_function_ref(&mut self, func: &FunctionRef) -> ConstantId {
    ModuleContext::lookup(&mut self.function_refs, func) as ConstantId
  }

  fn lookup_string_constant(&mut self, s: &String) -> ConstantId {
    ModuleContext::lookup(&mut self.string_constants, s) as ConstantId
  }

  fn lookup_shape(&mut self, shape: &Shape) -> ConstantId {
    ModuleContext::lookup(&mut self.shape_refs, shape) as ConstantId
  }

  fn lookup<T: Eq + Clone>(col: &mut Vec<T>, next: &T) -> usize {
    col.iter().position(|other| *other == *next)
      .or_else(move || {
        col.push(next.clone());
        Some(col.len() - 1)
      }).unwrap()
  }
}

struct FuncContext {
  max_locals: LocalId,
  free_slots: Vec<LocalId>,
  locals: HashMap<String, LocalId>,
}

impl FuncContext {

  fn new(args: &Vec<Parameter>) -> FuncContext {
    let mut locals = HashMap::new();

    let mut index = 0u16;
    for arg in args {
      locals.insert(arg.id.clone(), index);
      index += 1;
    }

    FuncContext {
      max_locals: index,
      free_slots: Vec::new(),
      locals,
    }
  }

  fn lookup_local(&mut self, name: &String) -> LocalId {
    self.locals.get(name)
      .map(|i| *i)
      .unwrap_or_else(move || {
        let id = self.free_slots.pop().unwrap_or_else(|| {
          let next = self.max_locals;
          self.max_locals += 1;
          next
        });
        self.locals.insert(name.clone(), id);
        id
      })
  }

  fn free(&mut self, name: &String) {
    let id = self.lookup_local(name);
    self.free_slots.push(id);
  }

}
