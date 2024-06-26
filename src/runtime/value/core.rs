use std::rc::Rc;

use crate::{
    binary::{ConstIndex, ConstValue},
    gc::{GcRef, GcRefVisitor, GcTraceable, PinnedGcRef},
    pure_values::{Float, Integer},
    runtime::{
        constants::{ConstLoader, ResolveFunc, ValueTable},
        context::ConstResolutionContext,
        environment::ModuleImportEnvironment,
        global_env::GlobalEnvLock,
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
    pub fn into_pinned(self) -> PinnedValue {
        PinnedValue(match self.0 {
            ValueInner::Integer(i) => PinnedValueInner::Integer(i),
            ValueInner::Float(f) => PinnedValueInner::Float(f),
            ValueInner::Bool(b) => PinnedValueInner::Bool(b),
            ValueInner::String(s) => PinnedValueInner::String(s),
            ValueInner::List(l) => PinnedValueInner::List(l.into_pinned()),
            ValueInner::Function(f) => PinnedValueInner::Function(f.into_pinned()),
        })
    }

    pub fn pin(&self) -> PinnedValue {
        PinnedValue(match &self.0 {
            ValueInner::Integer(i) => PinnedValueInner::Integer(i.clone()),
            ValueInner::Float(f) => PinnedValueInner::Float(f.clone()),
            ValueInner::Bool(b) => PinnedValueInner::Bool(*b),
            ValueInner::String(s) => PinnedValueInner::String(s.clone()),
            ValueInner::List(l) => PinnedValueInner::List(l.pin()),
            ValueInner::Function(f) => PinnedValueInner::Function(f.pin()),
        })
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
    consts: &[PinnedValue],
) -> Result<PinnedValue, RuntimeError> {
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
    ) -> Result<(PinnedValue, ResolveFunc<'a>), RuntimeError> {
        let (value, resolver) = match self {
            ConstValue::Bool(b) => (PinnedValueInner::Bool(*b), None),
            ConstValue::Integer(i) => (PinnedValueInner::Integer(i.clone()), None),
            ConstValue::Float(f) => (PinnedValueInner::Float(f.clone()), None),
            ConstValue::String(s) => (PinnedValueInner::String(s.clone()), None),
            ConstValue::List(list) => {
                let list_value = List::new(ctxt.env());
                let resolver: ResolveFunc = {
                    let list_value = list_value.clone();
                    Box::new(move |imports, vs| {
                        let list_elems = list_value;
                        for index in list {
                            list_elems.append(resolve_index(index, imports, vs)?);
                        }
                        Ok(())
                    })
                };

                (PinnedValueInner::List(list_value), Some(resolver))
            }
            ConstValue::Function(const_func) => {
                let (deferred, resolve_fn) = Function::new_managed_deferred(
                    ctxt.env(),
                    ctxt.module_globals().clone(),
                    Rc::new(ctxt.env().resolve_instructions(const_func.instructions())?),
                );
                let resolver: ResolveFunc = Box::new(move |imports, vs| {
                    let module_constants = const_func.module_constants();
                    let mut resolved_func_consts =
                        Vec::with_capacity(const_func.module_constants().len());
                    for index in module_constants {
                        resolved_func_consts.push(resolve_index(index, imports, vs)?);
                    }
                    resolve_fn(ValueTable::from_values(ctxt.env(), resolved_func_consts));
                    Ok(())
                });
                (PinnedValueInner::Function(deferred), Some(resolver))
            }
        };

        Ok((
            PinnedValue(value),
            resolver.unwrap_or(Box::new(|_, _| Ok(()))),
        ))
    }
}

#[derive(Clone)]
pub(crate) struct PinnedValue(PinnedValueInner);

impl PinnedValue {
    pub fn new_integer(i: Integer) -> Self {
        PinnedValue(PinnedValueInner::Integer(i))
    }

    pub fn new_float(f: Float) -> Self {
        PinnedValue(PinnedValueInner::Float(f))
    }

    pub fn new_bool(b: bool) -> Self {
        PinnedValue(PinnedValueInner::Bool(b))
    }

    pub fn new_string(s: ImmString) -> Self {
        PinnedValue(PinnedValueInner::String(s))
    }

    pub fn new_list(l: PinnedGcRef<List>) -> Self {
        PinnedValue(PinnedValueInner::List(l))
    }

    pub fn new_function(f: PinnedGcRef<Function>) -> Self {
        PinnedValue(PinnedValueInner::Function(f))
    }

    pub fn as_compact_integer(&self) -> Result<i64, RuntimeError> {
        match &self.0 {
            PinnedValueInner::Integer(i) => i
                .to_compact_integer()
                .ok_or_else(|| RuntimeError::new_conversion_error("Integer value is too large.")),
            _ => Err(RuntimeError::new_type_error("Value is not an integer.")),
        }
    }

    pub fn as_bool(&self) -> Result<bool, RuntimeError> {
        match &self.0 {
            PinnedValueInner::Bool(b) => Ok(*b),
            _ => Err(RuntimeError::new_type_error("Value is not a boolean.")),
        }
    }

    pub fn as_int(&self) -> Result<&Integer, RuntimeError> {
        match &self.0 {
            PinnedValueInner::Integer(i) => Ok(i),
            _ => Err(RuntimeError::new_type_error("Value is not an integer.")),
        }
    }

    pub fn as_float(&self) -> Result<&Float, RuntimeError> {
        match &self.0 {
            PinnedValueInner::Float(f) => Ok(f),
            _ => Err(RuntimeError::new_type_error("Value is not a float.")),
        }
    }

    pub fn as_function(&self) -> Result<&PinnedGcRef<Function>, RuntimeError> {
        match &self.0 {
            PinnedValueInner::Function(f) => Ok(f),
            _ => Err(RuntimeError::new_type_error("Value is not a function.")),
        }
    }

    pub fn as_list(&self) -> Result<&PinnedGcRef<List>, RuntimeError> {
        match &self.0 {
            PinnedValueInner::List(l) => Ok(l),
            _ => Err(RuntimeError::new_type_error("Value is not a list.")),
        }
    }

    pub fn as_str(&self) -> Result<&ImmString, RuntimeError> {
        match &self.0 {
            PinnedValueInner::String(s) => Ok(s),
            _ => Err(RuntimeError::new_type_error("Value is not a string.")),
        }
    }

    /// Returns true if the two values are the same concrete value, or are the same
    /// reference.
    pub fn ref_eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (PinnedValueInner::Bool(b1), PinnedValueInner::Bool(b2)) => b1 == b2,
            (PinnedValueInner::Integer(i1), PinnedValueInner::Integer(i2)) => i1 == i2,
            (PinnedValueInner::Float(f1), PinnedValueInner::Float(f2)) => f1 == f2,
            (PinnedValueInner::String(s1), PinnedValueInner::String(s2)) => s1 == s2,
            (PinnedValueInner::List(l1), PinnedValueInner::List(l2)) => PinnedGcRef::ref_eq(l1, l2),
            (PinnedValueInner::Function(f1), PinnedValueInner::Function(f2)) => {
                PinnedGcRef::ref_eq(f1, f2)
            }
            _ => false,
        }
    }

    pub fn add_owned(self, other: Self) -> Result<Self, RuntimeError> {
        match (self.0, other.0) {
            (PinnedValueInner::Integer(i1), PinnedValueInner::Integer(i2)) => {
                Ok(PinnedValue(PinnedValueInner::Integer(i1.add_owned(i2))))
            }
            (PinnedValueInner::Float(f1), PinnedValueInner::Float(f2)) => {
                Ok(PinnedValue(PinnedValueInner::Float(f1.add_owned(f2))))
            }
            _ => Err(RuntimeError::new_type_error(
                "Addition is only supported for integers and floats.",
            )),
        }
    }

    pub fn to_value(&self) -> Value {
        Value(match &self.0 {
            PinnedValueInner::Integer(i) => ValueInner::Integer(i.clone()),
            PinnedValueInner::Float(f) => ValueInner::Float(f.clone()),
            PinnedValueInner::Bool(b) => ValueInner::Bool(*b),
            PinnedValueInner::String(s) => ValueInner::String(s.clone()),
            PinnedValueInner::List(l) => ValueInner::List(l.to_ref()),
            PinnedValueInner::Function(f) => ValueInner::Function(f.to_ref()),
        })
    }

    pub fn into_value(self, env_lock: &GlobalEnvLock) -> Value {
        Value(match self.0 {
            PinnedValueInner::Integer(i) => ValueInner::Integer(i),
            PinnedValueInner::Float(f) => ValueInner::Float(f),
            PinnedValueInner::Bool(b) => ValueInner::Bool(b),
            PinnedValueInner::String(s) => ValueInner::String(s),
            PinnedValueInner::List(l) => ValueInner::List(l.into_ref(env_lock.guard())),
            PinnedValueInner::Function(f) => ValueInner::Function(f.into_ref(env_lock.guard())),
        })
    }
}

#[derive(Clone)]
enum PinnedValueInner {
    Integer(Integer),
    Float(Float),
    Bool(bool),
    String(ImmString),
    List(PinnedGcRef<List>),
    Function(PinnedGcRef<Function>),
}

impl From<Integer> for PinnedValue {
    fn from(i: Integer) -> Self {
        PinnedValue(PinnedValueInner::Integer(i))
    }
}

impl From<Float> for PinnedValue {
    fn from(f: Float) -> Self {
        PinnedValue(PinnedValueInner::Float(f))
    }
}

impl From<bool> for PinnedValue {
    fn from(b: bool) -> Self {
        PinnedValue(PinnedValueInner::Bool(b))
    }
}

impl From<ImmString> for PinnedValue {
    fn from(s: ImmString) -> Self {
        PinnedValue(PinnedValueInner::String(s))
    }
}

impl From<PinnedGcRef<List>> for PinnedValue {
    fn from(l: PinnedGcRef<List>) -> Self {
        PinnedValue(PinnedValueInner::List(l))
    }
}

impl From<PinnedGcRef<Function>> for PinnedValue {
    fn from(f: PinnedGcRef<Function>) -> Self {
        PinnedValue(PinnedValueInner::Function(f))
    }
}
