//! Loon has constants that represent constant values that can be resolved at
//! runtime. They don't themselves refer to Values, as that would require the
//! presence of a runtime, but they can be used to create Values.

use crate::binary::const_table::ConstValue;

use super::{
    context::ConstResolutionContext,
    environment::ModuleImportEnvironment,
    error::{Result, RuntimeError},
    value::Value,
};

pub type ResolveFunc<'a> = Box<dyn FnOnce(&ModuleImportEnvironment, &[Value]) -> Result<()> + 'a>;

pub trait ConstLoader {
    fn load<'a>(&'a self, ctxt: &'a ConstResolutionContext) -> Result<(Value, ResolveFunc<'a>)>;
}

pub fn resolve_constants<'a, T>(
    ctxt: &'a ConstResolutionContext,
    imports: &'a ModuleImportEnvironment,
    values: &'a [T],
) -> Result<Vec<Value>>
where
    T: ConstLoader,
{
    let mut resolved_values = Vec::with_capacity(values.len());
    let mut resolvers: Vec<ResolveFunc<'a>> = Vec::with_capacity(values.len());

    for value in values {
        let (value, resolver) = value.load(ctxt)?;
        resolved_values.push(value);
        resolvers.push(resolver);
    }

    for resolver in resolvers.into_iter() {
        resolver(imports, &resolved_values)?;
    }

    Ok(resolved_values)
}

#[derive(Clone)]
pub struct ValueTable(Vec<Value>);

impl ValueTable {
    /// Resolve a list of constant values into a new vector of runtime values.
    ///
    /// These values are resolved into the GlobalContext, so they will participate in
    /// garbage collection.
    ///
    /// We allow for self-referential constants and recursive constants via creating
    /// deferred references which will be resolved by the time that constant
    /// resolution completes.
    pub fn from_binary(const_table: &[ConstValue], ctxt: &ConstResolutionContext) -> Result<Self> {
        let values = resolve_constants(ctxt, ctxt.import_environment(), const_table)?;
        Ok(ValueTable(values))
    }

    pub fn at(&self, index: u32) -> Result<&Value> {
        self.0
            .get(usize::try_from(index).unwrap())
            .ok_or_else(|| RuntimeError::new_internal_error("Index out of bounds."))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        binary::const_table::{ConstIndex, ConstValue},
        pure_values::Float,
        runtime::{context::GlobalContext, modules::ModuleGlobals},
    };

    use super::*;

    #[test]
    fn build_simple_values() {
        let global_ctxt = GlobalContext::new();
        let module_globals = ModuleGlobals::from_size_empty(0);
        let import_environment = ModuleImportEnvironment::new(vec![]);
        let ctxt = ConstResolutionContext::new(&global_ctxt, &module_globals, &import_environment);
        let const_table = vec![
            ConstValue::Integer(42.into()),
            ConstValue::Float(Float::new(std::f64::consts::PI)),
            ConstValue::String("hello".into()),
        ];

        let resolved_values = ValueTable::from_binary(&const_table, &ctxt).unwrap();
        assert_eq!(resolved_values.0.len(), 3);

        match resolved_values.at(0).unwrap() {
            Value::Integer(i) => assert_eq!(*i, 42.into()),
            _ => panic!("Expected integer value."),
        }

        match resolved_values.at(1).unwrap() {
            Value::Float(f) => assert_eq!(f.value(), std::f64::consts::PI),
            _ => panic!("Expected float value."),
        }

        match resolved_values.at(2).unwrap() {
            Value::String(s) => assert_eq!(s.as_str(), "hello"),
            _ => panic!("Expected string value."),
        }
    }

    #[test]
    fn build_composite_value() {
        let global_ctxt = GlobalContext::new();
        let module_globals = ModuleGlobals::from_size_empty(0);
        let import_environment = ModuleImportEnvironment::new(vec![]);
        let ctxt = ConstResolutionContext::new(&global_ctxt, &module_globals, &import_environment);
        let values = vec![
            ConstValue::Integer(42.into()),
            ConstValue::List(vec![
                ConstIndex::ModuleConst(0),
                ConstIndex::ModuleConst(0),
                ConstIndex::ModuleConst(0),
            ]),
        ];

        let resolved_values = ValueTable::from_binary(&values, &ctxt).unwrap();
        assert_eq!(resolved_values.0.len(), 2);

        match resolved_values.at(1).unwrap() {
            Value::List(list) => {
                list.with(|l| {
                    assert_eq!(l.len(), 3);
                    for elem in l.iter() {
                        match elem {
                            Value::Integer(i) => assert_eq!(*i, 42.into()),
                            _ => panic!("Expected integer value."),
                        }
                    }
                });
            }
            _ => panic!("Expected integer value."),
        }
    }
}
