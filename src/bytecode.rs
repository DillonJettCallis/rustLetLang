use std::fs::File;

use simple_error::SimpleError;

use shapes::BaseShapeKind;
use shapes::Shape;

pub type LocalId = u16;
pub type ConstantId = u32;

pub struct AppDirectory {
  pub core_functions: Vec<String>,
  pub string_constants: Vec<String>,
  pub function_refs: Vec<BitFunction>,
  pub shape_refs: Vec<Shape>,
  pub source: String,
}

pub struct BitFunction {
  pub max_locals: LocalId,
  pub function_shape: Shape,
  pub body: Vec<Instruction>,
  pub source: Vec<SourcePoint>,
}

pub enum Instruction {
  NoOp, // 0 is an error to hopefully crash early on invalid bytecode.
  Duplicate,
  Pop,
  Swap,
  LoadConst {
    kind: u8,
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
  CallBuiltIn {
    func_id: ConstantId,
    shape_id: ConstantId,
  },
  CallStatic {
    func_id: ConstantId,
    shape_id: ConstantId,
  },
  CallDynamic {
    shape_id: ConstantId
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

pub struct SourcePoint {
  pub line: u32,
  pub column: u32,
}
