use std::rc::Rc;

use crate::refs::{GcRef, GcRefVisitor, GcTraceable};

use super::error::RuntimeError;
use num_traits::ToPrimitive;

mod function;
mod integer;
mod list;

pub use function::Function;
pub use integer::Integer;
pub use list::List;

#[derive(Clone, Debug)]
pub struct Float(f64);

impl GcTraceable for Float {
    fn trace<V>(&self, _visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        // No nested values to trace
    }
}

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
