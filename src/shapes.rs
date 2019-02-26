
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Shape {
  SimpleFunctionShape {
    args: Vec<Shape>,
    result: Box<Shape>
  },
  BaseShape {
    kind: BaseShapeKind
  },
  NamedShape {
    name: String
  },
  UnknownShape
}

impl Shape {
  pub fn pretty(&self) -> String {

    match self {
      Shape::SimpleFunctionShape{args, result} => {
        let arg_names = args.iter().map(|a| a.pretty()).collect::<Vec<String>>().join(", ");
        let result_name = result.pretty();

        format!("{{ {} -> {} }}", arg_names, result_name)
      }
      Shape::BaseShape{kind: BaseShapeKind::Float} => String::from("Float"),
      Shape::BaseShape{kind: BaseShapeKind::String} => String::from("String"),
      Shape::BaseShape{kind: BaseShapeKind::Unit} => String::from("Unit"),
      Shape::NamedShape{name} => name.clone(),
      Shape::UnknownShape => String::from("Unknown"),
    }
  }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BaseShapeKind {
  Float,
  String,
  Unit
}

pub fn shape_named(name: String) -> Shape {
  Shape::NamedShape {name}
}

pub fn shape_float() -> Shape {
  Shape::BaseShape { kind: BaseShapeKind::Float }
}

pub fn shape_string() -> Shape {
  Shape::BaseShape { kind: BaseShapeKind::String }
}

pub fn shape_unit() -> Shape {
  Shape::BaseShape { kind: BaseShapeKind::Unit }
}

pub fn shape_unknown() -> Shape {
  Shape::UnknownShape
}

