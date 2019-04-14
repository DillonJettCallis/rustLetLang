use std::rc::Rc;

use interpreter::RunFunction;
use shapes::BaseShapeKind;
use shapes::Shape;

#[derive(Clone, Debug)]
pub enum Value {
  Null,
  True,
  False,
  String(Rc<String>),
  Float(f64),
  Function(Rc<RunFunction>),
}

