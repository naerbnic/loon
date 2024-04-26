use std::rc::Rc;

use crate::{
    pure_values::{Float, Integer},
    refs::{GcRef, GcRefVisitor, GcTraceable},
};

use super::error::RuntimeError;
use num_traits::ToPrimitive;

mod function;
mod list;

pub use function::Function;
pub use list::List;

#[derive(Clone)]
pub enum Value {
    Integer(Integer),
    Float(Float),
    String(Rc<String>),
    List(GcRef<List>),
    Function(GcRef<Function>),
}

impl Value {
    pub fn as_compact_integer(&self) -> Result<i64, RuntimeError> {
        match self {
            Value::Integer(Integer::Compact(i)) => Ok(*i),
            Value::Integer(Integer::Big(i)) => i
                .to_i64()
                .ok_or_else(|| RuntimeError::new_conversion_error("Integer value is too large.")),
            _ => Err(RuntimeError::new_type_error("Value is not an integer.")),
        }
    }

    pub fn as_function(&self) -> Result<&GcRef<Function>, RuntimeError> {
        match self {
            Value::Function(f) => Ok(f),
            _ => Err(RuntimeError::new_type_error("Value is not a function.")),
        }
    }

    /// Returns true if the two values are the same concrete value, or are the same
    /// reference.
    pub fn ref_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Integer(i1), Value::Integer(i2)) => i1 == i2,
            (Value::Float(f1), Value::Float(f2)) => f1 as *const _ == f2 as *const _,
            (Value::String(s1), Value::String(s2)) => s1 as *const _ == s2 as *const _,
            (Value::List(l1), Value::List(l2)) => l1 as *const _ == l2 as *const _,
            (Value::Function(f1), Value::Function(f2)) => f1 as *const _ == f2 as *const _,
            _ => false,
        }
    }
}

impl GcTraceable for Value {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        match self {
            Value::Integer(_) | Value::Float(_) | Value::String(_) => {}
            Value::List(l) => visitor.visit(l),
            Value::Function(f) => visitor.visit(f),
        }
    }
}
