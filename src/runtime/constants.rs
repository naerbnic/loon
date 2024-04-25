//! Loon has constants that represent constant values that can be resolved at
//! runtime. They don't themselves refer to Values, as that would require the
//! presence of a runtime, but they can be used to create Values.

use std::rc::Rc;

use crate::runtime::value::{Function, List};

use super::{
    context::{GlobalContext, GlobalSymbol},
    error::RuntimeError,
    instructions::InstructionList,
    value::{Float, Integer, Value},
};

/// An index of a constant in the layers of constant values.
///
/// The layer is relative to the current context, with 0 being the current
/// context, 1 being the parent context, and so on.
///
/// The index is the index in the specified layer's values.
#[derive(Clone, Debug)]
struct LayerIndex {
    layer: usize,
    index: usize,
}

impl LayerIndex {
    pub fn new(layer: usize, index: usize) -> Self {
        LayerIndex { layer, index }
    }

    #[cfg(test)]
    pub fn new_in_base(index: usize) -> Self {
        LayerIndex { layer: 0, index }
    }
}

#[derive(Clone, Debug)]
pub enum ConstIndex {
    /// An index into the stack of constant tables.
    Local(LayerIndex),

    /// An index to be resolved globally by name.
    Global(GlobalSymbol),
}

#[derive(Clone, Debug)]
pub struct ConstFunction {
    /// Definitions of constants local to the function.
    const_table: Vec<ConstValue>,
    instructions: Rc<InstructionList>,
}

#[derive(Clone, Debug)]
pub enum ConstValue {
    /// An external ref to a constant.
    ///
    /// The resolution layer starts with the parent, so a layer of 0 refers to
    /// the parent context.
    ExternalRef(ConstIndex),
    Integer(Integer),
    Float(Float),
    String(String),
    List(Vec<ConstIndex>),
    Function(ConstFunction),
}

pub trait ConstResolver {
    fn resolve(&self, index: &ConstIndex) -> Result<Value, RuntimeError>;
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
    fn resolve(&self, index: &ConstIndex) -> Result<Value, RuntimeError> {
        match index {
            ConstIndex::Local(layer_index) => Err(RuntimeError::new_internal_error(
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
    fn resolve(&self, index: &ConstIndex) -> Result<Value, RuntimeError> {
        match index {
            ConstIndex::Local(layer_index) => {
                if layer_index.layer > 0 {
                    self.parent.resolve(&ConstIndex::Local(LayerIndex::new(
                        layer_index.layer - 1,
                        layer_index.index,
                    )))?;
                }
                let value = self
                    .values
                    .get(layer_index.index)
                    .ok_or_else(|| RuntimeError::new_internal_error("Invalid index."))?;
                Ok(value.clone())
            }
            ConstIndex::Global(_) => self.parent.resolve(index),
        }
    }
}

fn resolve_constants_impl<'a>(
    ctxt: &'a GlobalContext,
    const_resolver: &'a dyn ConstResolver,
    values: &'a [ConstValue],
) -> Result<Vec<Value>, RuntimeError> {
    type ResolverFn<'b> = Box<dyn FnOnce(&dyn ConstResolver) -> Result<(), RuntimeError> + 'b>;
    let mut resolved_values = Vec::with_capacity(values.len());
    let mut resolvers: Vec<Option<ResolverFn<'a>>> = Vec::with_capacity(values.len());

    for value in values {
        let (value, resolver) = match value {
            ConstValue::ExternalRef(index) => (const_resolver.resolve(index)?, None),
            ConstValue::Integer(i) => (Value::Integer(i.clone()), None),
            ConstValue::Float(f) => (Value::Float(f.clone()), None),
            ConstValue::String(s) => (Value::String(Rc::new(s.clone())), None),
            ConstValue::List(list) => {
                let (deferred, resolve_fn) = ctxt.create_deferred_ref();
                let resolver: ResolverFn<'a> = Box::new(move |vs| {
                    let mut list_elems = Vec::with_capacity(list.len());
                    for index in list {
                        list_elems.push(vs.resolve(&index)?);
                    }
                    resolve_fn(List::from_iter(list_elems));
                    Ok(())
                });

                (Value::List(deferred), Some(resolver))
            }
            ConstValue::Function(const_func) => {
                let (deferred, resolve_fn) = ctxt.create_deferred_ref();
                let resolver: ResolverFn<'a> = Box::new(move |vs| {
                    let resolved_func_consts =
                        resolve_constants_impl(ctxt, vs, &const_func.const_table[..])?;
                    resolve_fn(Function::new_managed(
                        resolved_func_consts,
                        const_func.instructions.clone(),
                    ));
                    Ok(())
                });
                (Value::Function(deferred), Some(resolver))
            }
        };
        resolved_values.push(value);
        resolvers.push(resolver);
    }

    let curr_layer = LocalResolver::new(const_resolver, &resolved_values);

    for resolver in resolvers.into_iter().flatten() {
        resolver(&curr_layer)?;
    }

    Ok(resolved_values)
}

#[derive(Clone)]
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
    pub fn resolve(&self, ctxt: &GlobalContext) -> Result<ValueTable, RuntimeError> {
        let curr_layer = GlobalResolver::new(ctxt);
        let values = resolve_constants_impl(ctxt, &curr_layer, &self.0)?;
        Ok(ValueTable(values))
    }
}

#[derive(Clone)]
pub struct ValueTable(Vec<Value>);

impl ValueTable {
    pub fn at(&self, index: usize) -> Result<&Value, RuntimeError> {
        self.0
            .get(index)
            .ok_or_else(|| RuntimeError::new_internal_error("Index out of bounds."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_simple_values() {
        let ctxt = GlobalContext::new();
        let const_resolver = &ctxt;
        let const_table = ConstTable::new(vec![
            ConstValue::Integer(Integer::Compact(42)),
            ConstValue::Float(Float::new(std::f64::consts::PI)),
            ConstValue::String("hello".to_string()),
        ]);

        let resolved_values = const_table.resolve(&ctxt).unwrap();
        assert_eq!(resolved_values.0.len(), 3);

        match resolved_values.at(0).unwrap() {
            Value::Integer(Integer::Compact(i)) => assert_eq!(*i, 42),
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
            ConstValue::Integer(Integer::Compact(42)),
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
                            Value::Integer(Integer::Compact(i)) => assert_eq!(*i, 42),
                            _ => panic!("Expected integer value."),
                        }
                    }
                });
            }
            _ => panic!("Expected integer value."),
        }
    }
}
