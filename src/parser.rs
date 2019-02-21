use shapes::*;
use ast::*;
use simple_error::*;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;

pub fn tokenize(src: &str) -> Result<Vec<Token>, SimpleError> {
  let mut source = Tokenizer::new(src)?;
  let mut tokens: Vec<Token> = Vec::new();

  loop {
    let next = source.parse();
    let is_done = next.kind == TokenKind::EOF;

    tokens.push(next);

    if is_done {
      break;
    }
  }
  Ok(tokens)
}

pub fn parse(src: &str) -> Result<Module, SimpleError> {
  let tokens = tokenize(src)?;
  let mut parser = Lexer { tokens, index: 0 };
  parser.parse_module()
}

const SUM_OPS: &'static [&'static str] = &["+", "-"];
const PROD_OPS: &'static [&'static str] = &["*", "/"];
const EQUAL_OPS: &'static [&'static str] = &["==", "!="];
const COMPARE_OPS: &'static [&'static str] = &["<", ">", "<=", ">="];

struct Lexer {
  tokens: Vec<Token>,
  index: usize,
}

impl Lexer {
  fn parse_module(&mut self) -> Result<Module, SimpleError> {
    let mut exports: Vec<Export> = Vec::new();
    let mut locals: Vec<Box<Expression>> = Vec::new();

    loop {
      let token = self.peek();

      match token.value.as_ref() {
        "export" => {
          let loc = token.location.clone();
          self.skip();
          let content = self.parse_function()?;
          exports.push(Export { loc, content });
        }
        "fun" => {
          locals.push(self.parse_function()?);
        }
        "<EOF>" => {
          return Ok(Module { exports, locals });
        }
        _ => {
          return Err(SimpleError::new(format!("Unexpected token: '{}' {}", token.value, token.location.pretty())));
        }
      }
    }
  }

  fn parse_function(&mut self) -> Result<Box<Expression>, SimpleError> {
    // Assume fun is already parsed

    let fun = self.next();
    assert!(fun.value == "fun");
    let loc = fun.location.clone();

    let id = self.expect_kind(TokenKind::Id)?.value;

    self.expect_literal("(")?;

    // TODO: Args
    let args: Vec<String> = Vec::new();

    self.expect_literal(")")?;

    self.expect_literal(":")?;

    let result_type_name = self.expect_kind(TokenKind::Id)?.value;

    self.expect_literal("=")?;

    let body = self.parse_expression()?;

    let shape = Shape::SimpleFunctionShape{ args: Vec::new(), result: Box::new(shape_named(result_type_name))};

    Ok(Box::new(Expression::FunctionDeclaration { shape, loc, id, args, body }))
  }

  fn parse_statement(&mut self) -> Result<Box<Expression>, SimpleError> {
    let maybe_key = self.peek();

    match maybe_key.value.as_ref() {
      "let" => self.parse_assignment(),
      "fun" => self.parse_function(),
      _ => self.parse_expression()
    }
  }

  fn parse_expression(&mut self) -> Result<Box<Expression>, SimpleError> {
    self.parse_ops()
  }

  fn parse_assignment(&mut self) -> Result<Box<Expression>, SimpleError> {
    // Assume let is already parsed

    let maybe_let = self.next();
    assert!(maybe_let.value == "let");

    let loc = maybe_let.location.clone();
    let id = self.expect_kind(TokenKind::Id)?.value;
    self.expect_literal("=")?;
    let body = self.parse_expression()?;
    let shape = shape_unknown();

    let maybe_colon = self.peek();

    if ";" == maybe_colon.value {
      self.skip()
    }

    Ok(Box::new(Expression::Assignment { shape, loc, id, body }))
  }

  fn parse_ops(&mut self) -> Result<Box<Expression>, SimpleError> {
    let start = |me: &mut Lexer| me.parse_block();
    let prod = |me: &mut Lexer| me.parse_binary_op(PROD_OPS, start);
    let sum = |me: &mut Lexer| me.parse_binary_op(SUM_OPS, prod);
    let compare = |me: &mut Lexer| me.parse_binary_op(COMPARE_OPS, sum);
    let equal = |me: &mut Lexer| me.parse_binary_op(EQUAL_OPS, compare);

    equal(self)
  }

  fn parse_binary_op<Next: Fn(&mut Lexer) -> Result<Box<Expression>, SimpleError>>(&mut self, ops: &[&str], next: Next) -> Result<Box<Expression>, SimpleError> {
    let mut left = next(self)?;

    let mut maybe_op = self.peek();

    while ops.contains(&maybe_op.value.as_ref()) {
      self.skip();
      let op = maybe_op.value;
      let loc = maybe_op.location.clone();
      let shape = shape_unknown();
      let right = next(self)?;

      left = Box::new(Expression::BinaryOp { shape, loc, left, right, op });
      maybe_op = self.peek();
    }

    Ok(left)
  }

  fn parse_block(&mut self) -> Result<Box<Expression>, SimpleError> {
    let maybe_brace = self.peek();

    if "{" == maybe_brace.value {
      let loc = maybe_brace.location.clone();
      let shape = shape_unknown();
      let mut body: Vec<Box<Expression>> = Vec::new();
      // Skip '{'
      self.skip();

      while "}" != self.peek().value {
        body.push(self.parse_statement()?)
      }
      // Skip '}'
      self.skip();

      Ok(Box::new(Expression::Block { loc, shape, body }))
    } else {
      self.parse_term()
    }
  }

  fn parse_term(&mut self) -> Result<Box<Expression>, SimpleError> {
    let term = self.next();
    let loc = term.location.clone();

    let raw = match term {
      Token { kind: TokenKind::Id, .. } => {
        let id = term.value;
        let shape = shape_unknown();
        Expression::Variable { id, shape, loc }
      }
      Token { kind: TokenKind::String, .. } => {
        let value = term.value;
        let shape = shape_string();
        Expression::StringLiteral { shape, loc, value }
      }
      Token { kind: TokenKind::Number, .. } => {
        let value = term.value.parse().or_else(|_| Err(SimpleError::new("Invalid float literal")))?;
        let shape = shape_float();
        Expression::NumberLiteral { shape, loc, value }
      }
      Token { kind: TokenKind::EOF, .. } => return Err(SimpleError::new("Unexpected <EOF>")),
      _ => return Err(SimpleError::new(format!("Unexpected Token: {:?}", term)))
    };

    Ok(Box::new(raw))
  }

  fn expect_literal(&mut self, value: &str) -> Result<Token, SimpleError> {
    let token = self.next();

    if token.value != value {
      return token.expected(value);
    } else {
      Ok(token)
    }
  }

  fn expect_kind(&mut self, kind: TokenKind) -> Result<Token, SimpleError> {
    let token = self.next();

    if token.kind != kind {
      return token.expected(format!("{:?}", kind).as_ref());
    } else {
      Ok(token)
    }
  }

  fn next(&mut self) -> Token {
    let result = self.tokens[self.index].clone();
    self.index = self.index + 1;
    result
  }

  fn peek(&self) -> Token {
    self.tokens[self.index].clone()
  }

  fn skip(&mut self) {
    self.index = self.index + 1;
  }

  fn prev(&mut self) {
    self.index = self.index - 1;
  }
}


const SINGLE_OPS: &'static str = "(){}<>[];";
const MERGE_OPS: &'static str = "=+-*/:";

struct Tokenizer {
  src: String,
  reader: CharReader<BufReader<File>>,
}

impl Tokenizer {
  fn new(src: &str) -> Result<Tokenizer, SimpleError> {
    let file = File::open(src).map_err(SimpleError::from)?;
    let buff = BufReader::new(file);
    let reader = CharReader::new(buff);

    Ok(Tokenizer { reader, src: String::from(src) })
  }

  fn point(&self) -> Location {
    let (x, y) = self.reader.point();
    Location { x, y, src: self.src.clone() }
  }

  fn parse(&mut self) -> Token {
    let is_space = |ch: char| ch.is_whitespace();
    let is_merge_op = |ch: char| MERGE_OPS.contains(ch);

    // Effectively skips whitespace by parsing and never saving it.
    self.parse_word(TokenKind::EOF, is_space, is_space);
    self.parse_word(TokenKind::Id, |ch| ch.is_alphabetic(), |ch| ch.is_alphanumeric())
      .or_else(|| self.parse_word(TokenKind::Symbol, |ch| SINGLE_OPS.contains(ch), |_ch| { false }))
      .or_else(|| self.parse_word(TokenKind::Symbol, is_merge_op, is_merge_op))
      .or_else(|| self.parse_word(TokenKind::Number, |ch| ch.is_numeric(), |ch| ch.is_numeric() || ch == '.'))
      .unwrap_or_else(|| Token { kind: TokenKind::EOF, value: String::from("<EOF>"), location: self.point() })
  }

  fn parse_word<L: Fn(char) -> bool, R: Fn(char) -> bool>(&mut self, kind: TokenKind, test_first: L, test: R) -> Option<Token> {
    match self.reader.current {
      Some(first) => if test_first(first) {
        let location = self.point();
        let mut value = String::new();
        value.push(first);

        loop {
          match self.reader.next() {
            Some(next) => if test(next) {
              value.push(next)
            } else {
              break;
            }
            None => break
          }
        }

        Some(Token { kind, value, location })
      } else {
        return None;
      }
      None => return None
    }
  }
}

#[derive(Debug, Clone)]
pub struct Token {
  pub kind: TokenKind,
  pub value: String,
  pub location: Location,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TokenKind {
  Id,
  Symbol,
  Number,
  String,
  EOF,
}

impl Token {
  pub fn expected<T>(&self, expected: &str) -> Result<T, SimpleError> {
    Err(SimpleError::new(format!("Unexpected '{}' found {}. Expected: {}", self.value, self.location.pretty(), expected)))
  }
}

struct CharReader<R: BufRead> {
  x: usize,
  y: usize,
  current: Option<char>,
  line: String,
  reader: R,
}

impl<R: BufRead> CharReader<R> {
  fn new(reader: R) -> CharReader<R> {
    let mut result = CharReader { x: 0, y: 0, current: None, line: String::new(), reader };
    result.next();
    result
  }

  fn next(&mut self) -> Option<char> {
    self.advance();
    self.current
  }

  fn advance(&mut self) {
    if self.x >= self.line.len() {
      self.line.clear();
      let char_count = self.reader.read_line(&mut self.line)
        .expect("Failed to parse file");
      if char_count == 0 {
        // End of file (even an empty line has a \n)
        self.current = None;
        return;
      }
      self.x = 0;
      self.y = self.y + 1;
    }

    self.current = self.line.chars().nth(self.x);
    self.x = self.x + 1;
  }

  fn point(&self) -> (usize, usize) {
    return (self.x, self.y);
  }
}
