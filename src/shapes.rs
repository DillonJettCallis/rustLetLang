
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Shape {
  GenericShapeConstructor {
    base: Box<Shape>,
    args: u8, // surely no one would ever need more than 256 type parameters?
  },
  GenericShape {
    base: Box<Shape>,
    args: Vec<Shape>,
  },
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
      Shape::GenericShapeConstructor{base, args} => {
        let arg_names = (0..*args).map(|_| "_").collect::<Vec<&str>>().join(", ");
        let base_name = base.pretty();

        format!("{}[{}]", base_name, arg_names)
      }
      Shape::GenericShape{base, args} => {
        let arg_names = args.iter().map(|a| a.pretty()).collect::<Vec<String>>().join(", ");
        let base_name = base.pretty();

        format!("{}[{}]", base_name, arg_names)
      },
      Shape::SimpleFunctionShape{args, result} => {
        let arg_names = args.iter().map(|a| a.pretty()).collect::<Vec<String>>().join(", ");
        let result_name = result.pretty();

        format!("{{ {} -> {} }}", arg_names, result_name)
      }
      Shape::BaseShape{kind: BaseShapeKind::Float} => String::from("Float"),
      Shape::BaseShape{kind: BaseShapeKind::String} => String::from("String"),
      Shape::BaseShape{kind: BaseShapeKind::Unit} => String::from("Unit"),
      Shape::BaseShape { kind: BaseShapeKind::List } => String::from("List"),
      Shape::NamedShape{name} => name.clone(),
      Shape::UnknownShape => String::from("Unknown"),
    }
  }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BaseShapeKind {
  Float,
  String,
  Unit,
  List
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

pub fn shape_list(arg: Shape) -> Shape {
  Shape::GenericShape {base: Box::new(Shape::BaseShape {kind: BaseShapeKind::List}), args: vec![arg]}
}

pub fn shape_unknown() -> Shape {
  Shape::UnknownShape
}

pub struct GenericShape {
  base: Shape,
  args: Vec<Shape>,
}