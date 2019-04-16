use bytecode::{BitModule, FunctionRef, BitPackage};
use interpreter::{RunFunction, Machine, NativeFunction};
use runtime::Value;
use simple_error::SimpleError;
use shapes::{Shape, shape_float, shape_boolean};
use std::collections::HashMap;
use std::rc::Rc;
use ast::Expression::BinaryOp;

pub fn core_runtime() -> BitPackage {

  let mut functions: HashMap<String, Box<RunFunction>> = HashMap::new();
  functions.insert(String::from("+"), Box::new(float_op("+", |l, r| l + r)));
  functions.insert(String::from("-"), Box::new(float_op("-", |l, r| l - r)));
  functions.insert(String::from("*"), Box::new(float_op("*", |l, r| l * r)));
  functions.insert(String::from("/"), Box::new(float_op("/", |l, r| l / r)));

  functions.insert(String::from("=="), Box::new(float_compare_op("==", |l, r| l == r)));
  functions.insert(String::from("!="), Box::new(float_compare_op("!=", |l, r| l != r)));
  functions.insert(String::from(">"), Box::new(float_compare_op(">", |l, r| l > r)));
  functions.insert(String::from(">="), Box::new(float_compare_op(">=", |l, r| l >= r)));
  functions.insert(String::from("<"), Box::new(float_compare_op("<", |l, r| l < r)));
  functions.insert(String::from("<="), Box::new(float_compare_op("<=", |l, r| l <= r)));

  let module = BitModule {
    functions,
    string_constants: vec![],
    function_refs: vec![],
    shape_refs: vec![]
  };

  let mut modules = HashMap::new();

  modules.insert(String::from("Core"), module);

  BitPackage {
    modules
  }
}


fn float_op<Op: Fn(f64, f64) -> f64>(name: &'static str, op: Op) -> impl RunFunction {
  let func = move |machine: &Machine, args: Vec<Value>| {
    if args.len() == 2 {
      if let Value::Float(first) = args[0] {
        if let Value::Float(second) = args[1] {
          let result = op(first, second);
          return Ok(Value::Float(result));
        }
      }
    }

    return Err(SimpleError::new(format!("{} takes exactly two float arguments", name)));
  };

  NativeFunction {
    func,
    func_ref: FunctionRef {
      package: String::from("Core"),
      module: String::from("Core"),
      name: String::from(name),

      shape: Shape::SimpleFunctionShape {
        args: vec![shape_float(), shape_float()],
        result: Box::new(shape_float()),
      },
    },
  }
}

fn float_compare_op<Op: Fn(f64, f64) -> bool>(name: &'static str, op: Op) -> impl RunFunction {
  let func = move |machine: &Machine, args: Vec<Value>| {
    if args.len() == 2 {
      if let Value::Float(first) = args[0] {
        if let Value::Float(second) = args[1] {
          let result = op(first, second);
          let value = if result {
            Value::True
          } else {
            Value::False
          };
          return Ok(value);
        }
      }
    }

    return Err(SimpleError::new(format!("{} takes exactly two float arguments", name)));
  };

  NativeFunction {
    func,
    func_ref: FunctionRef {
      package: String::from("Core"),
      module: String::from("Core"),
      name: String::from(name),

      shape: Shape::SimpleFunctionShape {
        args: vec![shape_float(), shape_float()],
        result: Box::new(shape_boolean()),
      },
    },
  }
}
