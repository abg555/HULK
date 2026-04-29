use std::fmt;

use crate::ast::TypeRef;

#[derive(Debug, Clone, PartialEq)]
pub enum SemanticType {
    Number,
    String,
    Boolean,
    Vector(Box<SemanticType>),
    Function(Vec<SemanticType>, Box<SemanticType>),
    Custom(String),
    Unknown,
}

impl SemanticType {
    pub fn from_type_ref(type_ref: &TypeRef) -> Self {
        match type_ref {
            TypeRef::Number => Self::Number,
            TypeRef::String => Self::String,
            TypeRef::Boolean => Self::Boolean,
            TypeRef::Vector(inner) => Self::Vector(Box::new(Self::from_type_ref(inner))),
            TypeRef::Function(params, ret) => Self::Function(
                params.iter().map(Self::from_type_ref).collect(),
                Box::new(Self::from_type_ref(ret)),
            ),
            TypeRef::Custom(name) => Self::Custom(name.clone()),
        }
    }

    pub fn is_assignable_from(&self, actual: &Self) -> bool {
        if matches!(self, Self::Unknown) || matches!(actual, Self::Unknown) {
            return true;
        }

        if self == actual {
            return true;
        }

        match (self, actual) {
            (Self::Vector(expected), Self::Vector(got)) => expected.is_assignable_from(got),
            (Self::Function(exp_params, exp_ret), Self::Function(got_params, got_ret)) => {
                exp_params.len() == got_params.len()
                    && exp_params
                        .iter()
                        .zip(got_params.iter())
                        .all(|(exp, got)| exp.is_assignable_from(got))
                    && exp_ret.is_assignable_from(got_ret)
            }
            _ => false,
        }
    }
}

impl fmt::Display for SemanticType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number => write!(f, "Number"),
            Self::String => write!(f, "String"),
            Self::Boolean => write!(f, "Boolean"),
            Self::Vector(inner) => write!(f, "{}[]", inner),
            Self::Function(params, ret) => {
                let params_text = params
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "({}) -> {}", params_text, ret)
            }
            Self::Custom(name) => write!(f, "{}", name),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}
