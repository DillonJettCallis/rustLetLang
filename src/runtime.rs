use std::rc::Rc;

use interpreter::RunFunction;
use shapes::BaseShapeKind;
use shapes::Shape;

#[derive(Clone)]
pub enum Value {
  String(Rc<String>),
  Float(f64),
  Function(Rc<RunFunction>),
}

