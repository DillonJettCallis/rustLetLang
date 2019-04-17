use std::collections::HashMap;
use std::rc::Rc;

use simple_error::SimpleError;

use ast::Expression::BinaryOp;
use bytecode::{BitModule, BitPackage, FunctionRef};
use interpreter::{Machine, NativeFunction, RunFunction};
use runtime::Value;
use shapes::{Shape, shape_boolean, shape_float};

pub fn core_runtime() -> BitPackage {
  let mut functions = HashMap::new();
  float_op(&mut functions, "+", |l, r| l + r);
  float_op(&mut functions, "-", |l, r| l - r);
  float_op(&mut functions, "*", |l, r| l * r);
  float_op(&mut functions, "/", |l, r| l / r);

  float_compare_op(&mut functions, "==", |l, r| l == r);
  float_compare_op(&mut functions, "!=", |l, r| l != r);
  float_compare_op(&mut functions, ">", |l, r| l > r);
  float_compare_op(&mut functions, ">=", |l, r| l >= r);
  float_compare_op(&mut functions, "<", |l, r| l < r);
  float_compare_op(&mut functions, "<=", |l, r| l <= r);

  let module = BitModule {
    functions,
    string_constants: vec![],
    function_refs: vec![],
    shape_refs: vec![],
  };

  let mut modules = HashMap::new();

  modules.insert(String::from("Core"), module);

  BitPackage {
    modules
  }
}

#[inline]
fn float_op<Op: Fn(f64, f64) -> f64 + 'static>(funcs: &mut HashMap<String, RunFunction>, name: &'static str, op_fun: Op) {
  op(funcs, name, op_fun, |result| Value::Float(result), shape_float())
}

#[inline]
fn float_compare_op<Op: Fn(f64, f64) -> bool + 'static>(funcs: &mut HashMap<String, RunFunction>, name: &'static str, op_fun: Op) {
  op(funcs, name, op_fun, |result| if result { Value::True } else { Value::False}, shape_boolean());
}

#[inline]
fn op<Result, Op: Fn(f64, f64) -> Result + 'static, Map: Fn(Result) -> Value + 'static>(funcs: &mut HashMap<String, RunFunction>, name: &'static str, op: Op, map: Map, result_shape: Shape) {
  let func = Box::new(move |machine: &Machine, args: Vec<Value>| {
    if args.len() == 2 {
      if let Value::Float(first) = args[0] {
        if let Value::Float(second) = args[1] {
          let result = op(first, second);
          return Ok(map(result));
        }
      }
    }

    return Err(SimpleError::new(format!("{} takes exactly two float arguments", name)));
  });

  let result = NativeFunction {
    func,
    func_ref: FunctionRef {
      package: String::from("Core"),
      module: String::from("Core"),
      name: String::from(name),

      shape: Shape::SimpleFunctionShape {
        args: vec![shape_float(), shape_float()],
        result: Box::new(result_shape),
      },
    },
  }.wrap();

  funcs.insert(String::from(name), result);
}
