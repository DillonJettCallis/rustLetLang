use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{self, ErrorKind, Write};
use std::rc::Rc;

use simple_error::SimpleError;

use interpreter::RunFunction;
use runtime::Value;
use shapes::BaseShapeKind;
use shapes::Shape;

pub type LocalId = u16;
pub type ConstantId = u32;

pub struct BitApplication {
  pub packages: HashMap<String, BitPackage>,
  pub main: (String, String),
}

impl BitApplication {

  pub fn new(main_package: String, main_module: String) -> BitApplication {
    BitApplication{
      packages: HashMap::new(),
      main: (main_package, main_module)
    }
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
  pub functions: HashMap<String, Rc<RunFunction>>,
  pub shape_refs: Vec<Shape>,
}

impl BitModule {

  fn pretty_constant(&self, kind: &LoadType, id: ConstantId) -> String {
    match kind {
      LoadType::String => self.string_constants[id as usize].clone(),
      LoadType::Function => self.function_refs[id as usize].name.clone()
    }
  }

  fn lookup_shape(&self, shape_id: ConstantId) -> Shape {
    self.shape_refs[shape_id as usize].clone()
  }

}

#[derive(Clone)]
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

pub struct BitFunction {
  pub package: String,
  pub module: String,
  pub name: String,

  pub max_locals: LocalId,
  pub shape: Shape,
  pub body: Vec<Instruction>,
  pub source: Vec<SourcePoint>,
}

impl BitFunction {

  pub fn debug(&self, module: &BitModule) -> Result<(), SimpleError> {
    let mut writer = io::stderr();

    writer.write_all(format!("{}: {}\n", self.name, self.shape.pretty()).as_bytes())
      .map_err(|err| SimpleError::from(err))?;

    Instruction::pretty_print(module, &self.body, &mut writer)?;

    writer.write_all(b"\n")
      .map_err(|err| SimpleError::from(err))
  }

}

pub enum LoadType {
  String,
  Function
}

impl LoadType {

  fn pretty(&self) -> &'static str {
    match self {
      LoadType::String => "String",
      LoadType::Function => "Function"
    }
  }

}

pub enum Instruction {
  NoOp, // 0 is an error to hopefully crash early on invalid bytecode.
  Duplicate,
  Pop,
  Swap,
  LoadConstNull,
  LoadConst {
    kind: LoadType,
    const_id: ConstantId
  },
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
  Return,
  IfEqual{jump: i32},
  IfNotEqual{jump: i32},
  IfTrue{jump: i32},
  IfFalse{jump: i32},
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
        Instruction::LoadConst {kind, const_id} => writer.write_all(format!("LoadConst({}, '{}')", kind.pretty(), module.pretty_constant(kind, *const_id)).as_bytes()),
        Instruction::LoadConstFloat {value} => writer.write_all(format!("LoadConstFloat({})", value).as_bytes()),
        Instruction::LoadValue {local} => writer.write_all(format!("LoadValue({})", local).as_bytes()),
        Instruction::StoreValue {local} => writer.write_all(format!("StoreValue({})", local).as_bytes()),
        Instruction::CallStatic {func_id} => writer.write_all(format!("CallStatic('{}')", module.pretty_constant(&LoadType::Function, *func_id)).as_bytes()),
        Instruction::CallDynamic {param_count} => writer.write_all(format!("CallDynamic({})", param_count).as_bytes()),
        Instruction::BuildClosure {param_count, func_id} => writer.write_all(format!("BuildClosure({}, '{}')", param_count, module.pretty_constant(&LoadType::Function, *func_id)).as_bytes()),
        Instruction::Return => writer.write_all(b"Return"),
        Instruction::IfEqual{jump} => writer.write_all(format!("IfEqual({})", jump).as_bytes()),
        Instruction::IfNotEqual{jump} => writer.write_all(format!("IfNotEqual({})", jump).as_bytes()),
        Instruction::IfTrue{jump} => writer.write_all(format!("IfTrue({})", jump).as_bytes()),
        Instruction::IfFalse{jump} => writer.write_all(format!("IfFalse({})", jump).as_bytes()),
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
