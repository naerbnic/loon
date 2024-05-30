//! Loon has constants that represent constant values that can be resolved at
//! runtime. They don't themselves refer to Values, as that would require the
//! presence of a runtime, but they can be used to create Values.

use crate::{
    binary::const_table::ConstValue,
    gc::{GcTraceable, PinnedGcRef},
};

use super::{
    context::ConstResolutionContext,
    environment::ModuleImportEnvironment,
    error::{Result, RuntimeError},
    global_env::GlobalEnv,
    value::{PinnedValue, Value},
};

pub type ResolveFunc<'a> =
    Box<dyn FnOnce(&ModuleImportEnvironment, &[PinnedValue]) -> Result<()> + 'a>;

pub trait ConstLoader {
    fn load<'a>(
        &'a self,
        ctxt: &'a ConstResolutionContext,
    ) -> Result<(PinnedValue, ResolveFunc<'a>)>;
}

pub fn resolve_constants<'a, T>(
    ctxt: &'a ConstResolutionContext,
    imports: &'a ModuleImportEnvironment,
    values: &'a [T],
) -> Result<Vec<PinnedValue>>
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

    for resolver in resolvers {
        resolver(imports, &resolved_values)?;
    }

    Ok(resolved_values)
}

#[derive(Clone)]
pub struct ValueTable(Vec<Value>);

impl ValueTable {
    /// Resolve a list of constant values into a new vector of runtime values.
    ///
    /// These values are resolved into the [`GlobalEnv`], so they will participate in
    /// garbage collection.
    ///
    /// We allow for self-referential constants and recursive constants via creating
    /// deferred references which will be resolved by the time that constant
    /// resolution completes.
    pub fn from_binary(
        const_table: &[ConstValue],
        ctxt: &ConstResolutionContext,
    ) -> Result<PinnedGcRef<Self>> {
        let values = resolve_constants(ctxt, ctxt.import_environment(), const_table)?;
        Ok(Self::from_values(ctxt.env(), values))
    }

    pub fn from_values(env: &GlobalEnv, values: Vec<PinnedValue>) -> PinnedGcRef<Self> {
        let lock = env.lock_collect();
        env.create_pinned_ref(ValueTable(
            values.into_iter().map(|v| v.into_value(&lock)).collect(),
        ))
    }

    pub fn at(&self, index: u32) -> Result<PinnedValue> {
        self.0
            .get(usize::try_from(index).unwrap())
            .map(Value::pin)
            .ok_or_else(|| RuntimeError::new_internal_error("Index out of bounds."))
    }
}

impl GcTraceable for ValueTable {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: crate::gc::GcRefVisitor,
    {
        for value in &self.0 {
            value.trace(visitor);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime::global_env::GlobalEnv;
    use crate::{
        binary::const_table::{ConstIndex, ConstValue},
        pure_values::Float,
        runtime::modules::ModuleGlobals,
    };

    use super::*;

    #[test]
    fn build_simple_values() -> anyhow::Result<()> {
        let const_table = vec![
            ConstValue::Integer(42.into()),
            ConstValue::Float(Float::new(std::f64::consts::PI)),
            ConstValue::String("hello".into()),
        ];

        let global_ctxt = GlobalEnv::new();
        let module_globals = ModuleGlobals::from_size_empty(&global_ctxt, 0);
        let import_environment = ModuleImportEnvironment::new(&global_ctxt, vec![]);
        let ctxt = ConstResolutionContext::new(&global_ctxt, &module_globals, &import_environment);

        let resolved_values = ValueTable::from_binary(&const_table, &ctxt).unwrap();
        assert_eq!(resolved_values.0.len(), 3);

        assert_eq!(resolved_values.at(0).unwrap().as_int()?, &42.into());
        assert_eq!(
            resolved_values.at(1).unwrap().as_float()?,
            &std::f64::consts::PI.into()
        );

        assert_eq!(resolved_values.at(2).unwrap().as_str()?, &"hello".into());
        Ok(())
    }

    #[test]
    fn build_composite_value() -> anyhow::Result<()> {
        let values = vec![
            ConstValue::Integer(42.into()),
            ConstValue::List(vec![
                ConstIndex::ModuleConst(0),
                ConstIndex::ModuleConst(0),
                ConstIndex::ModuleConst(0),
            ]),
        ];

        let global_ctxt = GlobalEnv::new();
        let module_globals = ModuleGlobals::from_size_empty(&global_ctxt, 0);
        let import_environment = ModuleImportEnvironment::new(&global_ctxt, vec![]);
        let ctxt = ConstResolutionContext::new(&global_ctxt, &module_globals, &import_environment);

        let resolved_values = ValueTable::from_binary(&values, &ctxt).unwrap();
        assert_eq!(resolved_values.0.len(), 2);

        let list = resolved_values.at(1).unwrap().as_list()?.clone();

        assert_eq!(list.len(), 3);
        for i in 0..3 {
            assert_eq!(list.at(i).as_int().unwrap(), &42.into());
        }

        Ok(())
    }
}
