//! Loon has constants that represent constant values that can be resolved at
//! runtime. They don't themselves refer to Values, as that would require the
//! presence of a runtime, but they can be used to create Values.

use crate::binary::const_table::{ConstIndex, ConstValue, LayerIndex};

use super::{
    context::GlobalContext,
    error::{Result, RuntimeError},
    value::Value,
};

pub type ResolveFunc<'a> = Box<dyn FnOnce(&dyn ConstResolver) -> Result<()> + 'a>;

pub trait ConstLoader {
    fn load<'a>(
        &'a self,
        const_resolver: &'a dyn crate::runtime::constants::ConstResolver,
        ctxt: &'a GlobalContext,
    ) -> Result<(Value, ResolveFunc<'a>)>;
}

pub trait ConstResolver {
    fn resolve(&self, index: &ConstIndex) -> Result<Value>;
}

pub struct GlobalResolver<'a> {
    ctxt: &'a GlobalContext,
}

impl<'a> GlobalResolver<'a> {
    pub fn new(ctxt: &'a GlobalContext) -> Self {
        GlobalResolver { ctxt }
    }
}

impl<'a> ConstResolver for GlobalResolver<'a> {
    fn resolve(&self, index: &ConstIndex) -> Result<Value> {
        match index {
            ConstIndex::Local(_layer_index) => Err(RuntimeError::new_internal_error(
                "Local resolution not supported.",
            )),
            ConstIndex::Global(name) => {
                let value = self
                    .ctxt
                    .lookup_symbol(name)
                    .ok_or_else(|| RuntimeError::new_internal_error("Symbol not found."))?;
                Ok(value)
            }
        }
    }
}

struct LocalResolver<'a> {
    parent: &'a dyn ConstResolver,
    values: &'a [Value],
}

impl<'a> LocalResolver<'a> {
    pub fn new(parent: &'a dyn ConstResolver, values: &'a [Value]) -> Self {
        LocalResolver { parent, values }
    }
}

impl<'a> ConstResolver for LocalResolver<'a> {
    fn resolve(&self, index: &ConstIndex) -> Result<Value> {
        match index {
            ConstIndex::Local(layer_index) => {
                if layer_index.layer() > 0 {
                    self.parent.resolve(&ConstIndex::Local(LayerIndex::new(
                        layer_index.layer() - 1,
                        layer_index.index(),
                    )))?;
                }
                let value = self
                    .values
                    .get(layer_index.index())
                    .ok_or_else(|| RuntimeError::new_internal_error("Invalid index."))?;
                Ok(value.clone())
            }
            ConstIndex::Global(_) => self.parent.resolve(index),
        }
    }
}

pub fn resolve_constants<'a, T>(
    ctxt: &'a GlobalContext,
    const_resolver: &'a dyn ConstResolver,
    values: &'a [T],
) -> Result<Vec<Value>>
where
    T: ConstLoader,
{
    type ResolverFn<'b> = Box<dyn FnOnce(&dyn ConstResolver) -> Result<()> + 'b>;
    let mut resolved_values = Vec::with_capacity(values.len());
    let mut resolvers: Vec<ResolverFn<'a>> = Vec::with_capacity(values.len());

    for value in values {
        let (value, resolver) = value.load(const_resolver, ctxt)?;
        resolved_values.push(value);
        resolvers.push(resolver);
    }

    let curr_layer = LocalResolver::new(const_resolver, &resolved_values);

    for resolver in resolvers.into_iter() {
        resolver(&curr_layer)?;
    }

    Ok(resolved_values)
}

#[derive(Clone, Debug)]
pub struct ConstTable(Vec<ConstValue>);

impl ConstTable {
    pub fn new(values: Vec<ConstValue>) -> Self {
        ConstTable(values)
    }

    /// Resolve a list of constant values into a new vector of runtime values.
    ///
    /// These values are resolved into the GlobalContext, so they will participate in
    /// garbage collection.
    ///
    /// We allow for self-referential constants and recursive constants via creating
    /// deferred references which will be resolved by the time that constant
    /// resolution completes.
    pub fn resolve(&self, ctxt: &GlobalContext) -> Result<ValueTable> {
        let curr_layer = GlobalResolver::new(ctxt);
        let values = resolve_constants(ctxt, &curr_layer, &self.0)?;
        Ok(ValueTable(values))
    }
}

#[derive(Clone)]
pub struct ValueTable(Vec<Value>);

impl ValueTable {
    pub fn at(&self, index: usize) -> Result<&Value> {
        self.0
            .get(index)
            .ok_or_else(|| RuntimeError::new_internal_error("Index out of bounds."))
    }
}

#[cfg(test)]
mod tests {
    use crate::pure_values::{Float, Integer};

    use super::*;

    #[test]
    fn build_simple_values() {
        let ctxt = GlobalContext::new();
        let const_table = ConstTable::new(vec![
            ConstValue::Integer(42.into()),
            ConstValue::Float(Float::new(std::f64::consts::PI)),
            ConstValue::String("hello".to_string()),
        ]);

        let resolved_values = const_table.resolve(&ctxt).unwrap();
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
        let ctxt = GlobalContext::new();
        let values = ConstTable::new(vec![
            ConstValue::Integer(42.into()),
            ConstValue::List(vec![
                ConstIndex::Local(LayerIndex::new_in_base(0)),
                ConstIndex::Local(LayerIndex::new_in_base(0)),
                ConstIndex::Local(LayerIndex::new_in_base(0)),
            ]),
        ]);

        let resolved_values = values.resolve(&ctxt).unwrap();
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
