use std::cmp::max;
use std::collections::HashMap;
use std::rc::Rc;

use simple_error::SimpleError;

use ast::Expression;
use ast::Module;
use bytecode::AppDirectory;
use bytecode::BitFunction;
use bytecode::FunctionRef;
use bytecode::Instruction;
use interpreter::RunFunction;
use shapes::Shape;
use bytecode::ConstantId;

pub struct Compiler {
  function_refs: HashMap<String, (u32, Shape)>,
  shape_refs: Vec<Shape>,
  string_constants: HashMap<String, u32>,
}

impl Compiler {
  pub fn compile(module: Module) -> Result<AppDirectory, SimpleError> {
    let me = Compiler {
      function_refs: HashMap::new(),
      shape_refs: Vec::new(),
      string_constants: HashMap::new(),
    };

    me.compile_app(module)
  }

  fn compile_app(mut self, module: Module) -> Result<AppDirectory, SimpleError> {
    let mut functions: HashMap<String, Box<RunFunction>> = HashMap::new();
    let mut context = FuncContext::new();

    for export in module.exports {
      let (id, bit_func) = self.compile_function(&mut context, &export.content)?;

      functions.insert(id, Box::new(bit_func));
    }

    for func in module.locals {
      let (id, bit_func) = self.compile_function(&mut context, &func)?;

      functions.insert(id, Box::new(bit_func));
    }

    let Compiler{string_constants, shape_refs, function_refs} = self;


    Ok(AppDirectory{
      string_constants: Compiler::reorder_string_constants(&string_constants)?,
      function_refs: Compiler::reorder_function_refs(&function_refs)?,
      functions,
      shape_refs
    })
  }

  fn compile_function(&mut self, context: &mut FuncContext, ex: &Expression) -> Result<(String, BitFunction), SimpleError> {
    if let Expression::FunctionDeclaration { shape, loc, id, args, body } = ex {
      context.max_locals = 0u16;
      context.locals.clear();

      let mut body = self.compile_expression(context, body)?;

      // if we don't end with a return, add one anyway.
      if let Some(Instruction::Return) = body.last() {} else {
        body.push(Instruction::Return);
      }

      return Ok((id.clone(), BitFunction {
        max_locals: context.max_locals + 1,
        shape: shape.clone(),
        body,
        source: vec![],
      }));
    }

    Err(SimpleError::new("Attempt to call compile_function with non-function expression"))
  }

  fn compile_expression(&mut self, context: &mut FuncContext, ex: &Expression) -> Result<Vec<Instruction>, SimpleError> {
    match ex {
      Expression::Assignment { shape, loc, id, body } => {
        let mut assign = self.compile_expression(context, body)?;
        let local = context.locals.len() as u16;
        assign.push(Instruction::StoreValue { local });
        context.locals.insert(id.clone(), (local, shape.clone()));
        return Ok(assign);
      }
      Expression::Variable { shape, loc, id } => {
        let (local, _) = context.locals.get(id)
          .ok_or_else(|| loc.error("Failed to get variable from context"))?;

        context.max_locals = max(local.clone(), context.max_locals);

        return Ok(vec![Instruction::LoadValue { local: local.clone() }]);
      }
      Expression::BinaryOp { shape, loc, op, left, right } => {
        let mut body = self.compile_expression(context, left)?;
        let mut other = self.compile_expression(context, right)?;
        body.append(&mut other);

        let name = String::from("Core.") + op;

        let ref_size = self.function_refs.len() as u32;
        let (func_id, _) = self.function_refs.entry(name)
          .or_insert_with(|| (ref_size, shape.clone()));

        body.push(Instruction::CallStatic { func_id: func_id.clone() });
        return Ok(body);
      },
      Expression::Call {shape, loc, func, args} => {
        let mut body = Vec::new();

        if let Expression::Variable {ref id, ..} = **func {

          for arg in args {
            let mut more = self.compile_expression(context, arg)?;
            body.append(&mut more);
          }

          let ref_size = self.function_refs.len() as u32;
          let (func_id, _) = self.function_refs.entry(id.clone())
            .or_insert_with(|| (ref_size, shape.clone()));

          body.push(Instruction::CallStatic {func_id: func_id.clone()})
        } else {
          let mut function = self.compile_expression(context, func)?;

          for arg in args {
            let mut more = self.compile_expression(context, arg)?;
            body.append(&mut more);
          }
          let shape_id = self.shape_refs.iter().position(|other| other == shape)
            .or_else(move || {
              self.shape_refs.push(shape.clone());
              Some(self.shape_refs.len() - 1)
            }).unwrap() as u32;

          body.push(Instruction::CallDynamic { shape_id })
        }

        Ok(body)
      },
      Expression::Block { shape, loc, body } => {
        let mut child_context = context.clone();
        let mut content: Vec<Instruction> = Vec::new();

        for next in body {
          let mut next_content = self.compile_expression(&mut child_context, next)?;
          content.append(&mut next_content);
        }

        if content.is_empty() {
          // If the block is empty, return a Null so there is something there.
          content.push(Instruction::LoadConstNull)
        }

        context.max_locals = max(child_context.max_locals, context.max_locals);

        return Ok(content);
      }
      Expression::StringLiteral { shape, loc, value } => {
        let str_size = self.string_constants.len() as u32;
        let const_id = self.string_constants.entry(value.clone())
          .or_insert_with(|| str_size)
          .clone();

        return Ok(vec![Instruction::LoadConst { kind: 0, const_id }]);
      }
      Expression::NumberLiteral { shape, loc, value } => {
        return Ok(vec![Instruction::LoadConstFloat { value: value.clone() }]);
      }

      _ => unimplemented!()
    }
  }

  fn reorder_string_constants(src_string_constants: &HashMap<String, u32>) -> Result<Vec<String>, SimpleError> {
    let mut raw_string_constants = Vec::with_capacity(src_string_constants.len());

    for (value, id) in src_string_constants {
      raw_string_constants.push((value.clone(), id.clone()));
    }

    raw_string_constants.sort_unstable_by_key(|(_, num)| num.clone());

    let mut string_constants: Vec<String> = Vec::with_capacity(src_string_constants.len());
    let mut index = 0u32;
    for (value, id) in raw_string_constants {
      if id != index {
        return Err(SimpleError::new("Invalid handling of string contants"));
      }

      index += 1;

      string_constants.push(value);
    }

    Ok(string_constants)
  }

  fn reorder_function_refs(src_function_refs: &HashMap<String, (u32, Shape)>) -> Result<Vec<FunctionRef>, SimpleError> {
    let mut raw_function_refs = Vec::with_capacity(src_function_refs.len());

    for (name, (id, shape)) in src_function_refs {
      raw_function_refs.push((name.clone(), id.clone(), shape.clone()));
    }

    raw_function_refs.sort_unstable_by_key(|(_, num, _)| num.clone());

    let mut function_refs: Vec<FunctionRef> = Vec::with_capacity(src_function_refs.len());
    let mut index = 0u32;
    for (name, id, shape) in raw_function_refs {
      if id != index {
        return Err(SimpleError::new("Invalid handling of string contants"));
      }

      index += 1;

      function_refs.push(FunctionRef{name, shape});
    }

    Ok(function_refs)
  }
}

#[derive(Clone)]
struct FuncContext {
  max_locals: u16,
  locals: HashMap<String, (u16, Shape)>,
}

impl FuncContext {
  fn new() -> FuncContext {
    FuncContext {
      max_locals: 0,
      locals: HashMap::new(),
    }
  }
}


