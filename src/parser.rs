use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;

use simple_error::*;

use ast::*;
use shapes::*;

pub fn lex(src: &str) -> Result<Vec<Token>, SimpleError> {
  let mut source = Lexer::new(src)?;
  let mut tokens: Vec<Token> = Vec::new();

  loop {
    let next = source.lex();
    let is_done = next.kind == TokenKind::EOF;

    tokens.push(next);

    if is_done {
      break;
    }
  }
  Ok(tokens)
}

pub fn parse(src: &str) -> Result<Module, SimpleError> {
  let tokens = lex(src)?;
  let mut parser = Parser { tokens, index: 0 };
  parser.parse_module()
}

const SUM_OPS: &'static [&'static str] = &["+", "-"];
const PROD_OPS: &'static [&'static str] = &["*", "/"];
const EQUAL_OPS: &'static [&'static str] = &["==", "!="];
const COMPARE_OPS: &'static [&'static str] = &["<", ">", "<=", ">="];

struct Parser {
  tokens: Vec<Token>,
  index: usize,
}

impl Parser {
  fn parse_module(&mut self) -> Result<Module, SimpleError> {
    let mut exports = Vec::new();
    let mut locals = Vec::new();

    loop {
      let token = self.peek();

      match token.value.as_ref() {
        "export" => {
          let loc = token.location.clone();
          self.skip();
          let content = self.parse_function(false)?;
          exports.push(Export { loc, content });
        }
        "fun" => {
          locals.push(self.parse_function(false)?);
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

  fn parse_function(&mut self, is_local: bool) -> Result<FunctionDeclarationEx, SimpleError> {
    // Assume fun is already parsed

    let fun = self.next();
    assert!(fun.value == "fun");
    let loc = fun.location.clone();

    let id = self.expect_kind(TokenKind::Id)?.value;

    self.expect_literal("(")?;

    let mut args = Vec::new();

    if !self.check_literal(")") {
      let arg_id = self.expect_kind(TokenKind::Id)?.value;
      self.expect_literal(":")?;
      let arg_shape = self.parse_type()?;
      args.push(Parameter{id: arg_id, shape: arg_shape});

      while self.check_literal(",") {
        let arg_id = self.expect_kind(TokenKind::Id)?.value;
        self.expect_literal(":")?;
        let arg_shape = self.parse_type()?;
        args.push(Parameter{id: arg_id, shape: arg_shape});
      }

      self.expect_literal(")")?;
    }

    self.expect_literal(":")?;

    let result = self.parse_type()?;

    self.expect_literal("=")?;

    let body = self.parse_expression()?;

    Ok(FunctionDeclarationEx{ result, loc, id, args, body, context: FunctionContext::new(is_local, false) })
  }

  fn parse_lambda(&mut self) -> Result<Expression, SimpleError> {
    // assume we've already checked and confirmed this is a lambda.

    let loc = self.peek_back().location;
    let mut args = Vec::new();

    let maybe_arrow = self.peek();

    if maybe_arrow.value != "->" && maybe_arrow.value != "=>" {
      let arg_id = self.expect_kind(TokenKind::Id)?.value;
      let arg_shape = if self.check_literal(":") {
        self.parse_type()?
      } else {
        shape_unknown()
      };
      args.push(Parameter{id: arg_id, shape: arg_shape});

      while self.check_literal(",") {
        let arg_id = self.expect_kind(TokenKind::Id)?.value;
        let arg_shape = if self.check_literal(":") {
          self.parse_type()?
        } else {
          shape_unknown()
        };
        args.push(Parameter{id: arg_id, shape: arg_shape});
      }
    }

    let result = if self.check_literal("->") {
      self.parse_type()?
    } else {
      shape_unknown()
    };

    let block_loc = self.expect_literal("=>")?.location;

    let mut body = Vec::new();

    while !self.check_literal("}") {
      body.push(self.parse_statement()?)
    }

    let block = Expression::Block (Box::new(BlockEx{
      loc: block_loc,
      shape: shape_unknown(),
      body
    }));

    Ok(FunctionDeclarationEx { result, loc, id: "<anon>".to_string(), args, body: block, context: FunctionContext::new(true, true) }.wrap())
  }

  fn parse_statement(&mut self) -> Result<Expression, SimpleError> {
    let maybe_key = self.peek();

    match maybe_key.value.as_ref() {
      "let" => self.parse_assignment(),
      "fun" => Ok(self.parse_function(true)?.wrap()),
      _ => self.parse_expression()
    }
  }

  fn parse_expression(&mut self) -> Result<Expression, SimpleError> {
    self.parse_ops()
  }

  fn parse_assignment(&mut self) -> Result<Expression, SimpleError> {
    // Assume let is already parsed

    let maybe_let = self.next();
    assert!(maybe_let.value == "let");

    let loc = maybe_let.location.clone();
    let id = self.expect_kind(TokenKind::Id)?.value;

    let shape = if self.check_literal(":") {
      self.parse_type()?
    } else {
      shape_unknown()
    };

    self.expect_literal("=")?;
    let body = self.parse_expression()?;

    let maybe_colon = self.peek();

    if ";" == maybe_colon.value {
      self.skip()
    }

    Ok(AssignmentEx { shape, loc, id, body }.wrap())
  }

  fn parse_ops(&mut self) -> Result<Expression, SimpleError> {
    let start = |me: &mut Parser| me.parse_call();
    let prod = |me: &mut Parser| me.parse_binary_op(PROD_OPS, start);
    let sum = |me: &mut Parser| me.parse_binary_op(SUM_OPS, prod);
    let compare = |me: &mut Parser| me.parse_binary_op(COMPARE_OPS, sum);
    let equal = |me: &mut Parser| me.parse_binary_op(EQUAL_OPS, compare);

    equal(self)
  }

  fn parse_binary_op<Next: Fn(&mut Parser) -> Result<Expression, SimpleError>>(&mut self, ops: &[&str], next: Next) -> Result<Expression, SimpleError> {
    let mut left = next(self)?;

    let mut maybe_op = self.peek();

    while ops.contains(&maybe_op.value.as_ref()) {
      self.skip();
      let op = maybe_op.value;
      let loc = maybe_op.location.clone();
      let shape = shape_unknown();
      let right = next(self)?;

      left = BinaryOpEx { shape, loc, left, right, op }.wrap();
      maybe_op = self.peek();
    }

    Ok(left)
  }

  fn parse_call(&mut self) -> Result<Expression, SimpleError> {
    let func = self.parse_block()?;

    if self.check_literal("(") {
      let mut args = Vec::new();

      if !self.check_literal(")") {
        args.push(self.parse_block()?);

        while self.check_literal(",") {
          args.push(self.parse_block()?);
        }

        self.expect_literal(")")?;
      }

      return Ok(CallEx {
        shape: shape_unknown(),
        loc: func.loc().clone(),
        func,
        args
      }.wrap())
    } else {
      return Ok(func);
    }
  }

  fn parse_block(&mut self) -> Result<Expression, SimpleError> {
    if self.check_literal("{") {
      if self.check_is_lambda() {
        return self.parse_lambda();
      }

      let loc = self.peek_back().location;
      let shape = shape_unknown();
      let mut body= Vec::new();

      while "}" != self.peek().value {
        body.push(self.parse_statement()?)
      }
      // Skip '}'
      self.skip();

      Ok(BlockEx { loc, shape, body }.wrap())
    } else {
      self.parse_term()
    }
  }

  fn parse_term(&mut self) -> Result<Expression, SimpleError> {
    let term = self.next();
    let loc = term.location.clone();

    let raw = match term {
      Token { kind: TokenKind::Id, .. } => {
        let id = term.value;
        let shape = shape_unknown();
        VariableEx { id, shape, loc }.wrap()
      }
      Token { kind: TokenKind::String, .. } => {
        let value = term.value;
        let shape = shape_string();
        StringLiteralEx { shape, loc, value }.wrap()
      }
      Token { kind: TokenKind::Number, .. } => {
        let value = term.value.parse().or_else(|_| Err(SimpleError::new("Invalid float literal")))?;
        let shape = shape_float();
        NumberLiteralEx { shape, loc, value }.wrap()
      }
      Token { kind: TokenKind::EOF, .. } => return Err(SimpleError::new("Unexpected <EOF>")),
      _ => return Err(SimpleError::new(format!("Unexpected Token: {:?}", term)))
    };

    Ok(raw)
  }

  fn parse_type(&mut self) -> Result<Shape, SimpleError> {
    self.parse_type_function()
  }

  fn parse_type_function(&mut self) -> Result<Shape, SimpleError> {
    if self.check_literal("{") {
      let mut args = Vec::new();

      if !self.check_literal("->") {
        args.push(self.parse_type()?);

        while self.check_literal(",") {
          args.push(self.parse_type()?);
        }

        self.expect_literal("->")?;
      }

      let result = Box::new(self.parse_type()?);

      self.expect_literal("}")?;

      return Ok(Shape::SimpleFunctionShape {
        args,
        result
      })
    } else {
      return self.parse_type_generic();
    }
  }

  fn parse_type_generic(&mut self) -> Result<Shape, SimpleError> {
    let base = self.parse_type_term()?;

    if self.check_literal("[") {
      let mut args = Vec::new();

      args.push(self.parse_type()?);

      while self.check_literal(",") {
        args.push(self.parse_type()?);
      }

      self.expect_literal("]")?;

      Ok(Shape::GenericShape {
        base: Box::new(base),
        args
      })
    } else {
      return Ok(base)
    }
  }

  fn parse_type_term(&mut self) -> Result<Shape, SimpleError> {
    let token = self.expect_kind(TokenKind::Id)?;
    Ok(shape_named(token.value))
  }

  /**
  Assume '{' is already parsed.
  We want to look ahead and see if we can find a => to denote this
  is a lambda or just a function.
  **/
  fn check_is_lambda(&self) -> bool {
    let mut index = self.index + 1;
    let mut opens = 1;

    while index < self.tokens.len() {
      let token = &self.tokens[index];
      index = index + 1;

      match token.value.as_ref() {
        "{" => {
          opens = opens + 1;
        }
        "}" => {
          opens = opens - 1;
        }
        "=>" => {
          return opens == 1;
        }
        _ => {}
      }
    }

    false
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

  fn check_literal(&mut self, value: &str) -> bool {
    let token = self.peek();

    if token.value != value {
      return false;
    } else {
      self.skip();
      return true;
    }
  }

  fn check_kind(&mut self, kind: TokenKind) -> bool {
    let token = self.next();

    if token.kind != kind {
      return false;
    } else {
      self.skip();
      return true;
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

  fn peek_back(&self) -> Token {
    self.tokens[self.index - 1].clone()
  }

  fn skip(&mut self) {
    self.index = self.index + 1;
  }

  fn prev(&mut self) {
    self.index = self.index - 1;
  }
}


const SINGLE_OPS: &'static str = "(){}[];,";
const MERGE_OPS: &'static str = "=+-*/:<>";

struct Lexer {
  src: String,
  reader: CharReader<BufReader<File>>,
}

impl Lexer {
  fn new(src: &str) -> Result<Lexer, SimpleError> {
    let file = File::open(src).map_err(SimpleError::from)?;
    let buff = BufReader::new(file);
    let reader = CharReader::new(buff);

    Ok(Lexer { reader, src: String::from(src) })
  }

  fn point(&self) -> Location {
    let (x, y) = self.reader.point();
    Location { x, y, src: self.src.clone() }
  }

  fn lex(&mut self) -> Token {
    let is_space = |ch: char| ch.is_whitespace();
    let is_merge_op = |ch: char| MERGE_OPS.contains(ch);

    // Effectively skips whitespace by parsing and never saving it.
    self.lex_word(TokenKind::EOF, is_space, is_space);
    self.lex_word(TokenKind::Id, |ch| ch.is_alphabetic(), |ch| ch.is_alphanumeric())
      .or_else(|| self.lex_word(TokenKind::Symbol, |ch| SINGLE_OPS.contains(ch), |_ch| { false }))
      .or_else(|| self.lex_word(TokenKind::Symbol, is_merge_op, is_merge_op))
      .or_else(|| self.lex_word(TokenKind::Number, |ch| ch.is_numeric(), |ch| ch.is_numeric() || ch == '.'))
      .unwrap_or_else(|| Token { kind: TokenKind::EOF, value: String::from("<EOF>"), location: self.point() })
  }

  fn lex_word<L: Fn(char) -> bool, R: Fn(char) -> bool>(&mut self, kind: TokenKind, test_first: L, test: R) -> Option<Token> {
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
