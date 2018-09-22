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
}

#[derive(Debug)]
pub enum Expression {
  FunctionDeclaration {
    shape: Shape,
    loc: Location,
    id: String,
    args: Vec<String>,
    body: Box<Expression>,
  },
  Assignment {
    shape: Shape,
    loc: Location,

    id: String,
    body: Box<Expression>,
  },
  Variable {
    shape: Shape,
    loc: Location,

    id: String,
  },
  BinaryOp {
    shape: Shape,
    loc: Location,

    op: String,
    left: Box<Expression>,
    right: Box<Expression>,
  },
  Block {
    shape: Shape,
    loc: Location,

    body: Vec<Box<Expression>>,
  },
  StringLiteral {
    shape: Shape,
    loc: Location,
    value: String,
  },
  NumberLiteral {
    shape: Shape,
    loc: Location,
    value: String,
  },
}

impl Expression {
  pub fn loc(&self) -> &Location {
    match self {
      Expression::FunctionDeclaration { loc, .. } => loc,
      Expression::Assignment { loc, .. } => loc,
      Expression::Variable { loc, .. } => loc,
      Expression::BinaryOp { loc, .. } => loc,
      Expression::Block { loc, .. } => loc,
      Expression::StringLiteral { loc, .. } => loc,
      Expression::NumberLiteral { loc, .. } => loc,
    }
  }

  pub fn shape(&self) -> &Shape {
    match self {
      Expression::FunctionDeclaration { shape, .. } => shape,
      Expression::Assignment { shape, .. } => shape,
      Expression::Variable { shape, .. } => shape,
      Expression::BinaryOp { shape, .. } => shape,
      Expression::Block { shape, .. } => shape,
      Expression::StringLiteral { shape, .. } => shape,
      Expression::NumberLiteral { shape, .. } => shape,
    }
  }
}

#[derive(Debug)]
pub struct Module {
  pub exports: Vec<Export>,
  pub locals: Vec<Box<Expression>>,
}

#[derive(Debug)]
pub struct Export {
  pub loc: Location,
  pub content: Box<Expression>,
}

