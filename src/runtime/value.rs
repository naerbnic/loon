use std::rc::Rc;

use crate::{
    function::Function,
    refs::{GcRef, GcRefVisitor, GcTraceable},
    Float, Integer, List,
};

use super::error::RuntimeError;
use num_traits::ToPrimitive;

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
