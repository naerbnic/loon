use std::rc::Rc;

use crate::{
    binary::{ConstIndex, ConstValue},
    gc::{GcRef, GcRefVisitor, GcTraceable},
    pure_values::{Float, Integer},
    runtime::{
        constants::{ConstLoader, ResolveFunc, ValueTable},
        context::ConstResolutionContext,
        environment::ModuleImportEnvironment,
        RuntimeError,
    },
    util::imm_string::ImmString,
};

use super::{Function, List};

#[derive(Clone)]
enum ValueInner {
    Integer(Integer),
    Float(Float),
    Bool(bool),
    String(ImmString),
    List(GcRef<List>),
    Function(GcRef<Function>),
}

#[derive(Clone)]
pub(crate) struct Value(ValueInner);

impl Value {
    pub fn new_integer(i: Integer) -> Self {
        Value(ValueInner::Integer(i))
    }

    pub fn new_float(f: Float) -> Self {
        Value(ValueInner::Float(f))
    }

    pub fn new_bool(b: bool) -> Self {
        Value(ValueInner::Bool(b))
    }

    pub fn new_string(s: ImmString) -> Self {
        Value(ValueInner::String(s))
    }

    pub fn new_list(l: GcRef<List>) -> Self {
        Value(ValueInner::List(l))
    }

    pub fn new_function(f: GcRef<Function>) -> Self {
        Value(ValueInner::Function(f))
    }

    pub fn as_compact_integer(&self) -> Result<i64, RuntimeError> {
        match &self.0 {
            ValueInner::Integer(i) => i
                .to_compact_integer()
                .ok_or_else(|| RuntimeError::new_conversion_error("Integer value is too large.")),
            _ => Err(RuntimeError::new_type_error("Value is not an integer.")),
        }
    }

    pub fn as_bool(&self) -> Result<bool, RuntimeError> {
        match &self.0 {
            ValueInner::Bool(b) => Ok(*b),
            _ => Err(RuntimeError::new_type_error("Value is not a boolean.")),
        }
    }

    pub fn as_int(&self) -> Result<&Integer, RuntimeError> {
        match &self.0 {
            ValueInner::Integer(i) => Ok(i),
            _ => Err(RuntimeError::new_type_error("Value is not an integer.")),
        }
    }

    pub fn as_float(&self) -> Result<&Float, RuntimeError> {
        match &self.0 {
            ValueInner::Float(f) => Ok(f),
            _ => Err(RuntimeError::new_type_error("Value is not a float.")),
        }
    }

    pub fn as_function(&self) -> Result<&GcRef<Function>, RuntimeError> {
        match &self.0 {
            ValueInner::Function(f) => Ok(f),
            _ => Err(RuntimeError::new_type_error("Value is not a function.")),
        }
    }

    pub fn as_list(&self) -> Result<&GcRef<List>, RuntimeError> {
        match &self.0 {
            ValueInner::List(l) => Ok(l),
            _ => Err(RuntimeError::new_type_error("Value is not a list.")),
        }
    }

    pub fn as_str(&self) -> Result<&ImmString, RuntimeError> {
        match &self.0 {
            ValueInner::String(s) => Ok(s),
            _ => Err(RuntimeError::new_type_error("Value is not a string.")),
        }
    }

    pub fn add_owned(self, other: Self) -> Result<Self, RuntimeError> {
        match (self.0, other.0) {
            (ValueInner::Integer(i1), ValueInner::Integer(i2)) => {
                Ok(Value(ValueInner::Integer(i1.add_owned(i2))))
            }
            (ValueInner::Float(f1), ValueInner::Float(f2)) => {
                Ok(Value(ValueInner::Float(f1.add_owned(f2))))
            }
            _ => Err(RuntimeError::new_type_error(
                "Addition is only supported for integers and floats.",
            )),
        }
    }

    /// Returns true if the two values are the same concrete value, or are the same
    /// reference.
    pub fn ref_eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (ValueInner::Bool(b1), ValueInner::Bool(b2)) => b1 == b2,
            (ValueInner::Integer(i1), ValueInner::Integer(i2)) => i1 == i2,
            (ValueInner::Float(f1), ValueInner::Float(f2)) => f1 == f2,
            (ValueInner::String(s1), ValueInner::String(s2)) => s1 == s2,
            (ValueInner::List(l1), ValueInner::List(l2)) => GcRef::ref_eq(l1, l2),
            (ValueInner::Function(f1), ValueInner::Function(f2)) => GcRef::ref_eq(f1, f2),
            _ => false,
        }
    }
}

impl GcTraceable for Value {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        match &self.0 {
            ValueInner::Integer(_)
            | ValueInner::Float(_)
            | ValueInner::String(_)
            | ValueInner::Bool(_) => {}
            ValueInner::List(l) => l.trace(visitor),
            ValueInner::Function(f) => f.trace(visitor),
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
    ) -> Result<(Value, ResolveFunc<'a>), RuntimeError> {
        let (value, resolver) = match self {
            ConstValue::Bool(b) => (ValueInner::Bool(*b), None),
            ConstValue::Integer(i) => (ValueInner::Integer(i.clone()), None),
            ConstValue::Float(f) => (ValueInner::Float(f.clone()), None),
            ConstValue::String(s) => (ValueInner::String(s.clone()), None),
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

                (ValueInner::List(list_value), Some(resolver))
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
                (ValueInner::Function(deferred), Some(resolver))
            }
        };

        Ok((Value(value), resolver.unwrap_or(Box::new(|_, _| Ok(())))))
    }
}
