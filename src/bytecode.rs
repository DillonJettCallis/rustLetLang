use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{self, ErrorKind, Write};
use std::rc::Rc;

use simple_error::SimpleError;
use serde::{Serialize, Deserialize};

use interpreter::RunFunction;
use runtime::Value;
use shapes::BaseShapeKind;
use shapes::Shape;
use std::hash::{Hash, Hasher};

pub type LocalId = u16;
pub type ConstantId = u32;

pub struct BitApplication {
  pub packages: HashMap<String, BitPackage>,
  pub main: FunctionRef,
}

impl BitApplication {

  pub fn new(main: FunctionRef) -> BitApplication {
    BitApplication {
      packages: HashMap::new(),
      main
    }
  }

  pub fn lookup_module(&self, func: &FunctionRef) -> Result<&BitModule, SimpleError> {
    self.packages.get(&func.package)
      .and_then(|package| package.modules.get(&func.module))
      .ok_or_else(|| SimpleError::new("FunctionRef Module lookup failed"))
  }

  pub fn lookup_function(&self, func: &FunctionRef) -> Result<&RunFunction, SimpleError> {
    self.packages.get(&func.package)
      .and_then(|package| package.modules.get(&func.module))
      .and_then(|module| module.functions.get(&func.name))
      .ok_or_else(|| SimpleError::new("FunctionRef Module lookup failed"))
  }
}

pub struct BitPackage {
  pub modules: HashMap<String, BitModule>,
}

impl BitPackage {

  pub fn new() -> BitPackage {
    BitPackage{modules: HashMap::new()}
  }
}

pub struct BitModule {
  pub string_constants: Vec<String>,
  pub function_refs: Vec<FunctionRef>,
  pub functions: HashMap<String, RunFunction>,
  pub shape_refs: Vec<Shape>,
}

impl BitModule {

  pub fn lookup_string(&self, id: ConstantId) -> Result<String, SimpleError> {
    Ok(self.string_constants.get(id as usize)
      .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid String constant id"))?
      .clone())
  }

  pub fn lookup_function(&self, id: ConstantId) -> Result<FunctionRef, SimpleError> {
    Ok(self.function_refs.get(id as usize)
      .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid Function constant id"))?
      .clone())
  }

  pub fn lookup_shape(&self, id: ConstantId) -> Result<Shape, SimpleError> {
    Ok(self.shape_refs.get(id as usize)
      .ok_or_else(|| SimpleError::new("Invalid bytecode. Invalid Shape constant id"))?
      .clone())
  }

  pub fn debug(&self) -> Result<(), SimpleError> {
    for raw in self.functions.values() {
      match raw {
        RunFunction::BitFunction(func) => func.debug(self)?,
        RunFunction::NativeFunction(func) => {
          let mut writer = io::stderr();

          writer.write_all(format!("{}: {}\n", func.func_ref.pretty(), func.func_ref.shape.pretty()).as_bytes())
            .map_err(|err| SimpleError::from(err))?;

          writer.write_all(b"  <native code>\n")
            .map_err(|err| SimpleError::from(err))?
        }
      }
    }

    Ok(())
  }

}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionRef {
  pub package: String,
  pub module: String,
  pub name: String,

  pub shape: Shape,
}

impl FunctionRef {

  pub fn pretty(&self) -> String {
    format!("{}::{}.{}", self.package, self.module, self.name)
  }

  pub fn result(&self) -> Shape {
    match &self.shape {
      Shape::SimpleFunctionShape{ result, ..} => *result.clone(),
      _ => self.shape.clone()
    }
  }
}

impl Hash for FunctionRef {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.package.hash(state);
    self.module.hash(state);
    self.name.hash(state);
  }
}

impl PartialEq for FunctionRef {
  fn eq(&self, other: &FunctionRef) -> bool {
    self.module == other.module && self.name == other.name && self.package == other.package
  }
}

impl Eq for FunctionRef {}

pub struct BitFunction {
  pub func_ref: FunctionRef,

  pub max_locals: LocalId,
  pub body: Vec<Instruction>,
  pub source: Vec<SourcePoint>,
}

impl BitFunction {

  pub fn debug(&self, module: &BitModule) -> Result<(), SimpleError> {
    let mut writer = io::stderr();

    writer.write_all(format!("{}: {}\n", self.func_ref.pretty(), self.func_ref.shape.pretty()).as_bytes())
      .map_err(|err| SimpleError::from(err))?;

    Instruction::pretty_print(module, &self.body, &mut writer)?;

    writer.write_all(b"\n")
      .map_err(|err| SimpleError::from(err))
  }

}

pub enum Instruction {
  NoOp, // 0 is an error to hopefully crash early on invalid bytecode.
  Duplicate,
  Pop,
  Swap,
  LoadConstNull,
  LoadConstTrue,
  LoadConstFalse,
  LoadConstString {const_id: ConstantId},
  LoadConstFunction {const_id: ConstantId},
  LoadConstFloat {
    value: f64
  },
  LoadValue {
    local: LocalId
  },
  StoreValue {
    local: LocalId
  },
  CallStatic {
    func_id: ConstantId,
  },
  CallDynamic {
    param_count: LocalId,
  },
  BuildClosure {
    param_count: LocalId,
    func_id: ConstantId,
  },
  BuildRecursiveFunction,
  Return,
  Branch{jump: i32},
  Jump{jump: i32},
  Debug,
  Error
}

impl Instruction {

  fn pretty_print<Writer: Write>(module: &BitModule, block: &Vec<Instruction>, writer: &mut Writer) -> Result<(), SimpleError> {

    for (index, next) in block.iter().enumerate() {
      writer.write_all(format!("  {}: ", index).as_bytes()).map_err(|err| SimpleError::from(err))?;

      match next {
        Instruction::NoOp => writer.write_all(b"NoOp"),
        Instruction::Duplicate => writer.write_all(b"Duplicate"),
        Instruction::Pop => writer.write_all(b"Pop"),
        Instruction::Swap => writer.write_all(b"Swap"),
        Instruction::LoadConstNull => writer.write_all(b"LoadConstNull"),
        Instruction::LoadConstTrue => writer.write_all(b"LoadConstTrue"),
        Instruction::LoadConstFalse => writer.write_all(b"LoadConstFalse"),
        Instruction::LoadConstString {const_id} => writer.write_all(format!("LoadConstString('{}')", module.lookup_string(*const_id)?).as_bytes()),
        Instruction::LoadConstFunction {const_id} => writer.write_all(format!("LoadConstFunction('{}')", module.lookup_function(*const_id)?.pretty()).as_bytes()),
        Instruction::LoadConstFloat {value} => writer.write_all(format!("LoadConstFloat({})", value).as_bytes()),
        Instruction::LoadValue {local} => writer.write_all(format!("LoadValue({})", local).as_bytes()),
        Instruction::StoreValue {local} => writer.write_all(format!("StoreValue({})", local).as_bytes()),
        Instruction::CallStatic {func_id} => writer.write_all(format!("CallStatic('{}')", module.lookup_function(*func_id)?.pretty()).as_bytes()),
        Instruction::CallDynamic {param_count} => writer.write_all(format!("CallDynamic({})", param_count).as_bytes()),
        Instruction::BuildClosure {param_count, func_id} => writer.write_all(format!("BuildClosure({}, '{}')", param_count, module.lookup_function(*func_id)?.pretty()).as_bytes()),
        Instruction::BuildRecursiveFunction => writer.write_all(b"BuildRecursiveFunction"),
        Instruction::Return => writer.write_all(b"Return"),
        Instruction::Branch{jump} => writer.write_all(format!("Branch({})", jump).as_bytes()),
        Instruction::Jump{jump} => writer.write_all(format!("Jump({})", jump).as_bytes()),
        Instruction::Debug => writer.write_all(b"Debug"),
        Instruction::Error => writer.write_all(b"Error"),
      }.map_err(|err| SimpleError::from(err))?;

      writer.write_all(b"\n").map_err(|err| SimpleError::from(err))?;
    }

    Ok(())
  }

}

pub struct SourcePoint {
  pub line: u32,
  pub column: u32,
}
