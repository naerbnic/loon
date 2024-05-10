use std::rc::Rc;

use crate::{
    binary::{ConstIndex, ConstValue},
    gc::{GcRef, GcRefVisitor, GcTraceable},
    pure_values::{Float, Integer},
    util::imm_string::ImmString,
};

use super::{
    constants::{ConstLoader, ResolveFunc, ValueTable},
    context::ConstResolutionContext,
    environment::ModuleImportEnvironment,
    error::RuntimeError,
};

mod function;
mod list;
pub use self::function::native::NativeFunctionResult;
pub(crate) use function::native::{
    NativeFunctionContext, NativeFunctionPtr, NativeFunctionResultInner,
};
pub(crate) use function::Function;
pub(crate) use list::List;

#[derive(Clone)]
pub(crate) enum Value {
    Integer(Integer),
    Float(Float),
    Bool(bool),
    String(ImmString),
    List(GcRef<List>),
    Function(Function),
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

    pub fn as_bool(&self) -> Result<bool, RuntimeError> {
        match self {
            Value::Bool(b) => Ok(*b),
            _ => Err(RuntimeError::new_type_error("Value is not a boolean.")),
        }
    }

    pub fn as_int(&self) -> Result<&Integer, RuntimeError> {
        match self {
            Value::Integer(i) => Ok(i),
            _ => Err(RuntimeError::new_type_error("Value is not an integer.")),
        }
    }

    pub fn as_function(&self) -> Result<&Function, RuntimeError> {
        match self {
            Value::Function(f) => Ok(f),
            _ => Err(RuntimeError::new_type_error("Value is not a function.")),
        }
    }

    pub fn as_list(&self) -> Result<&GcRef<List>, RuntimeError> {
        match self {
            Value::List(l) => Ok(l),
            _ => Err(RuntimeError::new_type_error("Value is not a list.")),
        }
    }

    pub fn as_str(&self) -> Result<&ImmString, RuntimeError> {
        match self {
            Value::String(s) => Ok(s),
            _ => Err(RuntimeError::new_type_error("Value is not a string.")),
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
            (Value::List(l1), Value::List(l2)) => GcRef::ref_eq(l1, l2),
            (Value::Function(f1), Value::Function(f2)) => Function::ref_eq(f1, f2),
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
            Value::List(l) => l.trace(visitor),
            Value::Function(f) => f.trace(visitor),
        }
    }
}
fn resolve_index(
    const_index: &ConstIndex,
    imports: &ModuleImportEnvironment,
    consts: &[Value],
) -> Result<Value, RuntimeError> {
    match const_index {
        ConstIndex::ModuleConst(index) => consts
            .get(usize::try_from(*index).unwrap())
            .cloned()
            .ok_or_else(|| RuntimeError::new_internal_error("Invalid index.")),
        ConstIndex::ModuleImport(index) => imports.get_import(*index),
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
                let list_value = ctxt.env_lock().create_ref(List::new());
                let resolver: ResolveFunc = {
                    let list_value = list_value.clone();
                    Box::new(move |imports, vs| {
                        let list_elems = list_value.borrow();
                        for index in list {
                            list_elems.append(resolve_index(index, imports, vs)?);
                        }
                        Ok(())
                    })
                };

                (Value::List(list_value), Some(resolver))
            }
            ConstValue::Function(const_func) => {
                let (deferred, resolve_fn) = Function::new_managed_deferred(
                    ctxt.env_lock(),
                    ctxt.module_globals().clone(),
                    Rc::new(
                        ctxt.env_lock()
                            .resolve_instructions(const_func.instructions())?,
                    ),
                );
                let resolver: ResolveFunc = Box::new(move |imports, vs| {
                    let module_constants = const_func.module_constants();
                    let mut resolved_func_consts =
                        Vec::with_capacity(const_func.module_constants().len());
                    for index in module_constants {
                        resolved_func_consts.push(resolve_index(index, imports, vs)?);
                    }
                    resolve_fn(ValueTable::from_values(resolved_func_consts));
                    Ok(())
                });
                (Value::Function(deferred), Some(resolver))
            }
        };

        Ok((value, resolver.unwrap_or(Box::new(|_, _| Ok(())))))
    }
}
