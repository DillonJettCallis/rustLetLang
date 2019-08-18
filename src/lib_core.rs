use std::collections::HashMap;
use std::rc::Rc;

use simple_error::SimpleError;

use ast::Expression::BinaryOp;
use bytecode::{BitModule, BitPackage, FunctionRef};
use interpreter::{Machine, NativeFunction, RunFunction};
use runtime::{Value, ListValue};
use shapes::{Shape, shape_list};
use std::borrow::Borrow;

pub fn core_runtime() -> BitPackage {
  let mut modules = HashMap::new();

  modules.insert(String::from("Core"), core_module());
  modules.insert(String::from("List"), list_module());

  BitPackage {
    modules
  }
}

fn core_module() -> BitModule {
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

  BitModule {
    functions,
    string_constants: vec![],
    function_refs: vec![],
    shape_refs: vec![],
  }
}

fn list_module() -> BitModule {
  let mut functions = HashMap::new();
  let float_list = shape_list(shape!(Float));
  let mapper_shape = Shape::SimpleFunctionShape {
    args: vec![shape!(Float)],
    result: Box::new(shape!(Float))
  };
  let reducer_shape = Shape::SimpleFunctionShape {
    args: vec![shape!(Float), shape!(Float)],
    result: Box::new(shape!(Float))
  };

  exact(&mut functions, "List", "new", 0, |_, _| Ok(Value::List(Rc::new(ListValue::new(shape!(Float))))), Shape::SimpleFunctionShape {
    args: vec![],
    result: Box::new(float_list.clone()),
  });

  exact(&mut functions, "List", "append", 2, |_, args| {
    if let Value::List(list) = &args[0] {
      if let Value::Float(num) = args[1] {
        let mut copy = list.copy_contents();
        copy.push(Value::Float(num));
        Ok(Value::List(Rc::new(ListValue{ contents: copy, shape: list.shape.clone()})))
      } else {
        Err(SimpleError::new("List.append second argument must be a float"))
      }
    } else {
      Err(SimpleError::new("List.append first argument must be a list"))
    }
  }, Shape::SimpleFunctionShape {
    args: vec![float_list.clone(), shape!(Float)],
    result: Box::new(float_list.clone()),
  });

  exact(&mut functions, "List", "map", 2, |machine, args| {
    if let Value::List(list) = args[0].clone() {
      if let Value::Function(mapper) = &args[1] {
        let mut result = Vec::with_capacity(list.contents.len());

        for next in 0..list.contents.len() {
          result.push(machine.execute_handle(mapper.clone(), vec![ list.contents[next].clone() ])?);
        }

        Ok(Value::List(Rc::new(ListValue{ contents: result, shape: list.shape.clone()})))
      } else {
        Err(SimpleError::new("List.map second argument must be a function"))
      }
    } else {
      Err(SimpleError::new("List.map first argument must be a list"))
    }
  }, mapper_shape);

  exact(&mut functions, "List", "fold", 3, |machine, args| {
    if let Value::List(list) = args[0].clone() {
      if let Value::Float(init) = args[1] {
        if let Value::Function(mapper) = &args[2] {
          let mut result = init;

          for item in &list.contents {
            if let Value::Float(next) = machine.execute_handle(mapper.clone(), vec![Value::Float(result), item.clone()])? {
              result = next
            } else {
              return Err(SimpleError::new("List.fold callback must return a float"))
            }
          }

          Ok(Value::Float(result))
        } else {
          Err(SimpleError::new("List.fold third argument must be a function"))
        }
      } else {
        Err(SimpleError::new("List.fold second argument must be a float"))
      }
    } else {
      Err(SimpleError::new("List.fold first argument must be a list"))
    }
  }, Shape::SimpleFunctionShape {
    args: vec![float_list.clone(), shape!(Float), reducer_shape],
    result: Box::new(float_list.clone())
  });

  BitModule {
    functions,
    string_constants: vec![],
    function_refs: vec![],
    shape_refs: vec![],
  }
}

#[inline]
fn float_op<Op: Fn(f64, f64) -> f64 + 'static>(funcs: &mut HashMap<String, RunFunction>, name: &'static str, op_fun: Op) {
  op(funcs, name, op_fun, |result| Value::Float(result), shape!(Float))
}

#[inline]
fn float_compare_op<Op: Fn(f64, f64) -> bool + 'static>(funcs: &mut HashMap<String, RunFunction>, name: &'static str, op_fun: Op) {
  op(funcs, name, op_fun, |result| if result { Value::True } else { Value::False}, shape!(Boolean));
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
        args: vec![shape!(Float), shape!(Float)],
        result: Box::new(result_shape),
      },
    },
  }.wrap();

  funcs.insert(String::from(name), result);
}

#[inline]
fn exact<Op: Fn(&Machine, Vec<Value>) -> Result<Value, SimpleError> + 'static>(funcs: &mut HashMap<String, RunFunction>, module: &'static str, name: &'static str, arg_count: usize, op: Op, shape: Shape) {
  let func = Box::new(move |machine: &Machine, args: Vec<Value>| {
    if args.len() == arg_count {
      return op(machine, args)
    }

    return Err(SimpleError::new(format!("{}.{} takes exactly two float arguments", module, name)));
  });

  let result = NativeFunction {
    func,
    func_ref: FunctionRef {
      package: String::from("Core"),
      module: String::from(module),
      name: String::from(name),

      shape,
    },
  }.wrap();

  funcs.insert(String::from(name), result);
}
