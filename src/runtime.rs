use std::rc::Rc;

use interpreter::FunctionHandle;
use shapes::BaseShapeKind;
use shapes::Shape;

#[derive(Clone, Debug)]
pub enum Value {
  Null,
  True,
  False,
  String(Rc<String>),
  Float(f64),
  Function(Rc<FunctionHandle>),
  List(Rc<ListValue>)
}

#[derive(Clone, Debug)]
pub struct ListValue {
  pub contents: Vec<Value>,
  pub shape: Shape,
}

impl ListValue {

  pub fn new(shape: Shape) -> ListValue {
    ListValue {
      contents: Vec::new(),
      shape
    }
  }

  pub fn copy_contents(&self) -> Vec<Value> {
    self.contents.clone()
  }

}
