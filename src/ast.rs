use serde::{Deserialize, Serialize};
use simple_error::SimpleError;

use shapes::*;

#[derive(Debug, Clone)]
pub struct Location {
  pub src: String,
  pub x: usize,
  pub y: usize,
}

impl Location {
  pub fn pretty(&self) -> String {
    format!("at file: {}, line: {}, column: {}", self.src, self.y, self.x)
  }

  pub fn fail<T>(&self, message: &str) -> Result<T, SimpleError> {
    Err(SimpleError::new(format!("{} {}", message, self.pretty())))
  }

  pub fn error(&self, message: &str) -> SimpleError {
    SimpleError::new(format!("{} {}", message, self.pretty()))
  }
}

pub enum Expression {
  NoOp(Location),
  FunctionDeclaration(Box<FunctionDeclarationEx>),
  Assignment(Box<AssignmentEx>),
  Variable(Box<VariableEx>),
  BinaryOp(Box<BinaryOpEx>),
  Call(Box<CallEx>),
  If(Box<IfEx>),
  Block(Box<BlockEx>),
  StringLiteral(Box<StringLiteralEx>),
  NumberLiteral(Box<NumberLiteralEx>),
  BooleanLiteral(Location, bool),
}

impl Expression {
  pub fn loc(&self) -> &Location {
    match self {
      Expression::NoOp(loc) => loc,
      Expression::FunctionDeclaration(ex) => &ex.loc,
      Expression::Assignment(ex) => &ex.loc,
      Expression::Variable(ex) => &ex.loc,
      Expression::BinaryOp(ex) => &ex.loc,
      Expression::Call(ex) => &ex.loc,
      Expression::If(ex) => &ex.loc,
      Expression::Block(ex) => &ex.loc,
      Expression::StringLiteral(ex) => &ex.loc,
      Expression::NumberLiteral(ex) => &ex.loc,
      Expression::BooleanLiteral(loc, _) => loc,
    }
  }

  pub fn shape(&self) -> Shape {
    match self {
      Expression::NoOp(_) => shape_unit(),
      Expression::FunctionDeclaration(ex) => ex.shape(),
      Expression::Assignment(ex) => ex.shape.clone(),
      Expression::Variable(ex) => ex.shape.clone(),
      Expression::BinaryOp(ex) => ex.shape.clone(),
      Expression::Call(ex) => ex.shape.clone(),
      Expression::If(ex) => ex.shape.clone(),
      Expression::Block(ex) => ex.shape.clone(),
      Expression::StringLiteral(ex) => ex.shape.clone(),
      Expression::NumberLiteral(ex) => ex.shape.clone(),
      Expression::BooleanLiteral(..) => shape_boolean(),
    }
  }
}
pub struct FunctionContext {
  pub is_lambda: bool,
  pub is_local: bool,
  pub is_recursive: bool,
  pub closures: Vec<Parameter>,
}

impl FunctionContext {
  pub fn new(is_local: bool, is_lambda: bool) -> FunctionContext {
    FunctionContext {
      is_local,
      is_lambda,
      is_recursive: false,
      closures: Vec::new(),
    }
  }

  pub fn set_closures(&self, closures: Vec<Parameter>) -> FunctionContext {
    FunctionContext {
      is_local: self.is_local,
      is_lambda: self.is_lambda,
      is_recursive: self.is_recursive,
      closures,
    }
  }

  pub fn set_is_recursive(&self, is_recursive: bool) -> FunctionContext {
    FunctionContext {
      is_local: self.is_local,
      is_lambda: self.is_lambda,
      is_recursive,
      closures: self.closures.clone(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
  pub id: String,
  pub shape: Shape,
}

impl Parameter {

  pub fn pretty(&self) -> String {
    format!("{}: {}", self.id, self.shape.pretty())
  }

}

pub struct FunctionDeclarationEx {
  pub result: Shape,
  pub loc: Location,
  pub id: String,
  pub args: Vec<Parameter>,
  pub body: Expression,
  pub context: FunctionContext,
}

pub struct AssignmentEx {
  pub shape: Shape,
  pub loc: Location,

  pub id: String,
  pub body: Expression,
}

pub struct VariableEx {
  pub shape: Shape,
  pub loc: Location,

  pub id: String,
}

pub struct BinaryOpEx {
  pub shape: Shape,
  pub loc: Location,

  pub op: String,
  pub left: Expression,
  pub right: Expression,
}

pub struct CallEx {
  pub shape: Shape,
  pub loc: Location,

  pub func: Expression,
  pub args: Vec<Expression>,
}

pub struct IfEx {
  pub shape: Shape,
  pub loc: Location,

  pub condition: Expression,
  pub then_block: Expression,
  pub else_block: Expression,
}

pub struct BlockEx {
  pub shape: Shape,
  pub loc: Location,

  pub body: Vec<Expression>,
}

pub struct StringLiteralEx {
  pub shape: Shape,
  pub loc: Location,

  pub value: String,
}

pub struct NumberLiteralEx {
  pub shape: Shape,
  pub loc: Location,

  pub value: f64,
}

pub struct Module {
  pub package: String,
  pub name: String,
  pub functions: Vec<FunctionDeclaration>,
}

pub struct FunctionDeclaration {
  pub visibility: Visibility,
  pub ex: FunctionDeclarationEx,
}

pub enum Visibility {
  Private,
  Protected,
  Internal,
  Public
}

impl FunctionDeclarationEx {
  pub fn wrap(self) -> Expression {
    Expression::FunctionDeclaration(Box::new(self))
  }

  pub fn shape(&self) -> Shape {
    Shape::SimpleFunctionShape {args: self.args.iter().map(|arg| arg.shape.clone()).collect(), result: Box::new(self.result.clone()) }
  }
}

impl AssignmentEx {
  pub fn wrap(self) -> Expression {
    Expression::Assignment(Box::new(self))
  }
}

impl VariableEx {
  pub fn wrap(self) -> Expression {
    Expression::Variable(Box::new(self))
  }
}

impl BinaryOpEx {
  pub fn wrap(self) -> Expression {
    Expression::BinaryOp(Box::new(self))
  }
}

impl CallEx {
  pub fn wrap(self) -> Expression {
    Expression::Call(Box::new(self))
  }
}

impl IfEx {
  pub fn wrap(self) -> Expression {
    Expression::If(Box::new(self))
  }
}

impl BlockEx {
  pub fn wrap(self) -> Expression {
    Expression::Block(Box::new(self))
  }
}

impl StringLiteralEx {
  pub fn wrap(self) -> Expression {
    Expression::StringLiteral(Box::new(self))
  }
}

impl NumberLiteralEx {
  pub fn wrap(self) -> Expression {
    Expression::NumberLiteral(Box::new(self))
  }
}