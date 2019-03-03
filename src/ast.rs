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
  FunctionDeclaration(Box<FunctionDeclarationEx>),
  Assignment(Box<AssignmentEx>),
  Variable(Box<VariableEx>),
  BinaryOp(Box<BinaryOpEx>),
  Call(Box<CallEx>),
  Block(Box<BlockEx>),
  StringLiteral(Box<StringLiteralEx>),
  NumberLiteral(Box<NumberLiteralEx>),
}

impl Expression {
  pub fn loc(&self) -> &Location {
    match self {
      Expression::FunctionDeclaration(ex) => &ex.loc,
      Expression::Assignment(ex) => &ex.loc,
      Expression::Variable(ex) => &ex.loc,
      Expression::BinaryOp(ex) => &ex.loc,
      Expression::Call(ex) => &ex.loc,
      Expression::Block(ex) => &ex.loc,
      Expression::StringLiteral(ex) => &ex.loc,
      Expression::NumberLiteral(ex) => &ex.loc,
    }
  }

  pub fn shape(&self) -> &Shape {
    match self {
      Expression::FunctionDeclaration(ex) => &ex.shape,
      Expression::Assignment(ex) => &ex.shape,
      Expression::Variable(ex) => &ex.shape,
      Expression::BinaryOp(ex) => &ex.shape,
      Expression::Call(ex) => &ex.shape,
      Expression::Block(ex) => &ex.shape,
      Expression::StringLiteral(ex) => &ex.shape,
      Expression::NumberLiteral(ex) => &ex.shape,
    }
  }
}


pub struct FunctionDeclarationEx {
  pub shape: Shape,
  pub loc: Location,
  pub id: String,
  pub args: Vec<String>,
  pub body: Expression,
  pub closures: Vec<String>,
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
  pub exports: Vec<Export>,
  pub locals: Vec<FunctionDeclarationEx>,
}

pub struct Export {
  pub loc: Location,
  pub content: FunctionDeclarationEx,
}

impl FunctionDeclarationEx {
  pub fn wrap(self) -> Expression {
    Expression::FunctionDeclaration(Box::new(self))
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