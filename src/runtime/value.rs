use std::rc::Rc;

use crate::{
    binary::ConstValue,
    pure_values::{Float, Integer},
    refs::{GcRef, GcRefVisitor, GcTraceable},
    util::imm_string::ImmString,
};

use super::{
    constants::{ConstLoader, ResolveFunc, ValueTable},
    context::ConstResolutionContext,
    error::RuntimeError,
};

mod function;
mod list;

pub(crate) use function::Function;
pub(crate) use list::List;

#[derive(Clone)]
pub(crate) enum Value {
    Integer(Integer),
    Float(Float),
    Bool(bool),
    String(ImmString),
    List(GcRef<List>),
    Function(GcRef<Function>),
}

impl Value {
    pub fn as_compact_integer(&self) -> Result<i64, RuntimeError> {
        match self {
            Value::Integer(i) => i
                .to_compact_integer()
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

    pub fn as_int(&self) -> Result<&Integer, RuntimeError> {
        match self {
            Value::Integer(i) => Ok(i),
            _ => Err(RuntimeError::new_type_error("Value is not an integer.")),
        }
    }

    /// Returns true if the two values are the same concrete value, or are the same
    /// reference.
    pub fn ref_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Bool(b1), Value::Bool(b2)) => b1 == b2,
            (Value::Integer(i1), Value::Integer(i2)) => i1 == i2,
            (Value::Float(f1), Value::Float(f2)) => f1 == f2,
            (Value::String(s1), Value::String(s2)) => s1 == s2,
            (Value::List(l1), Value::List(l2)) => std::ptr::eq(l1 as *const _, l2 as *const _),
            (Value::Function(f1), Value::Function(f2)) => {
                std::ptr::eq(f1 as *const _, f2 as *const _)
            }
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
            Value::Integer(_) | Value::Float(_) | Value::String(_) | Value::Bool(_) => {}
            Value::List(l) => visitor.visit(l),
            Value::Function(f) => visitor.visit(f),
        }
    }
}

impl ConstLoader for ConstValue {
    fn load<'a>(
        &'a self,
        ctxt: &'a ConstResolutionContext,
    ) -> Result<(crate::runtime::value::Value, ResolveFunc<'a>), RuntimeError> {
        let (value, resolver) = match self {
            ConstValue::Bool(b) => (Value::Bool(*b), None),
            ConstValue::Integer(i) => (Value::Integer(i.clone()), None),
            ConstValue::Float(f) => (Value::Float(f.clone()), None),
            ConstValue::String(s) => (Value::String(s.clone()), None),
            ConstValue::List(list) => {
                let (deferred, resolve_fn) = ctxt.global_context().create_deferred_ref();
                let resolver: ResolveFunc = Box::new(move |imports, vs| {
                    let mut list_elems = Vec::with_capacity(list.len());
                    for index in list {
                        list_elems.push(index.resolve(imports, vs)?);
                    }
                    resolve_fn(List::from_iter(list_elems));
                    Ok(())
                });

                (Value::List(deferred), Some(resolver))
            }
            ConstValue::Function(const_func) => {
                let (deferred, resolve_fn) = ctxt.global_context().create_deferred_ref();
                let resolver: ResolveFunc = Box::new(move |imports, vs| {
                    let module_constants = const_func.module_constants();
                    let mut resolved_func_consts =
                        Vec::with_capacity(const_func.module_constants().len());
                    for index in module_constants {
                        resolved_func_consts.push(index.resolve(imports, vs)?);
                    }
                    resolve_fn(Function::new_managed(
                        ctxt.module_globals().clone(),
                        ValueTable::from_values(resolved_func_consts),
                        Rc::new(
                            ctxt.global_context()
                                .resolve_instructions(const_func.instructions())?,
                        ),
                    ));
                    Ok(())
                });
                (Value::Function(deferred), Some(resolver))
            }
        };

        Ok((value, resolver.unwrap_or(Box::new(|_, _| Ok(())))))
    }
}
