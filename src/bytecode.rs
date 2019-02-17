use std::fs::File;

use simple_error::SimpleError;

use shapes::BaseShapeKind;
use shapes::Shape;
use std::collections::HashMap;
use interpreter::RunFunction;

pub type LocalId = u16;
pub type ConstantId = u32;

pub struct AppDirectory {
  pub string_constants: Vec<String>,
  pub function_refs: Vec<FunctionRef>,
  pub functions: HashMap<String, Box<RunFunction>>,
  pub shape_refs: Vec<Shape>,
  pub source: String,
}

pub struct FunctionRef {
  pub name: String,
  pub shape: Shape,
}

pub struct BitFunction {
  pub max_locals: LocalId,
  pub shape: Shape,
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
  CallStatic {
    func_id: ConstantId,
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
