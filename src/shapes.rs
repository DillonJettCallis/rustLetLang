use serde::{Serialize, Deserialize};
use ast::Location;
use typechecker::fill_shape;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
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
      Shape::BaseShape{kind: BaseShapeKind::Boolean} => String::from("Boolean"),
      Shape::BaseShape{kind: BaseShapeKind::Float} => String::from("Float"),
      Shape::BaseShape{kind: BaseShapeKind::String} => String::from("String"),
      Shape::BaseShape{kind: BaseShapeKind::Unit} => String::from("Unit"),
      Shape::BaseShape { kind: BaseShapeKind::List } => String::from("List"),
      Shape::NamedShape{name} => name.clone(),
      Shape::UnknownShape => String::from("Unknown"),
    }
  }

  pub fn fill_shape_native(self) -> Shape {
    fill_shape(self, &Location { src: String::from("<native>"), x: 0, y: 0, }).unwrap()
  }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum BaseShapeKind {
  Boolean,
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

pub fn shape_boolean() -> Shape {
  Shape::BaseShape { kind: BaseShapeKind::Boolean }
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

#[macro_export]
macro_rules! shape {
  (@list $base:ident [ $($inner:tt)+ ], $($tail:tt)+ ) => ({ // List [ <anything> ], <anything>
      let mut rest: Vec<Shape> = shape!(@list $($tail)*);
      rest.push(shape!($base [ $($inner)+ ]));
      rest
    });
  (@list $base:ident [ $($inner:tt)+ ]) => (vec![shape!($base [ $($inner)+ ] )]); // List [ <anything> ]
  (@list $base:ident, $($tail:tt)+ ) => ({ // Float, <anything>
      let mut rest: Vec<Shape> = shape!(@list $($tail)*);
      rest.push(shape!($base));
      rest
    });
  (@list $base:ident) => (vec![shape!($base)]); // Float
  ($base:ident [ $($inner:tt)+ ] ) => ({
      let mut args = shape!(@list $($inner)+);
      args.reverse();
      Shape::GenericShape {base: Box::new( shape!( $base )  ), args}
    });
  (Boolean) => (Shape::BaseShape { kind: BaseShapeKind::Boolean });
  (Float) => (Shape::BaseShape { kind: BaseShapeKind::Float });
  (String) => (Shape::BaseShape { kind: BaseShapeKind::String });
  (Unit) => (Shape::BaseShape { kind: BaseShapeKind::Unit });
  (List) => (Shape::BaseShape { kind: BaseShapeKind::List });
}
