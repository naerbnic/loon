//! Loon has constants that represent constant values that can be resolved at
//! runtime. They don't themselves refer to Values, as that would require the
//! presence of a runtime, but they can be used to create Values.

use std::rc::Rc;

use crate::{
    refs::GcContext,
    runtime::value::{Function, List},
};

use super::{
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

    pub fn new_in_base(index: usize) -> Self {
        LayerIndex { layer: 0, index }
    }
}

#[derive(Clone, Debug)]
pub enum ConstIndex {
    /// An index into the stack of constant tables.
    Local(LayerIndex),

    /// An index to be resolved globally by name.
    Global(Rc<String>),
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
    fn resolve_name(&self, ctxt: &GcContext, name: &str) -> Result<Value, RuntimeError>;
}

#[derive(Clone, Copy)]
struct ValueLayer<'a> {
    parent: Option<&'a ValueLayer<'a>>,
    values: &'a [Value],
}

impl<'a> ValueLayer<'a> {
    pub fn new(parent: Option<&'a ValueLayer<'a>>, values: &'a [Value]) -> Self {
        ValueLayer { parent, values }
    }

    pub fn make_child<'p>(&'p self, values: &'p [Value]) -> ValueLayer<'p> {
        ValueLayer {
            parent: Some(self),
            values,
        }
    }

    pub fn parent(&self) -> Option<&ValueLayer<'_>> {
        self.parent
    }

    pub fn get(&self, layer_index: &LayerIndex) -> Result<Value, RuntimeError> {
        let mut layer = self;
        for _ in 0..layer_index.layer {
            layer = layer
                .parent
                .ok_or_else(|| RuntimeError::new_internal_error("Invalid layer index."))?;
        }

        let value = layer
            .values
            .get(layer_index.index)
            .ok_or_else(|| RuntimeError::new_internal_error("Invalid index."))?
            .clone();
        Ok(value)
    }
}

/// Resolve a list of constant values into a new vector of runtime values.
///
/// These values are resolved into the GcContext, so they will participate in
/// garbage collection.
///
/// We allow for self-referential constants and recursive constants via creating
/// deferred references which will be resolved by the time that constant
/// resolution completes.

pub fn resolve_constants<'a>(
    ctxt: &'a GcContext,
    const_resolver: &'a dyn ConstResolver,
    values: &'a [ConstValue],
) -> Result<Vec<Value>, RuntimeError> {
    resolve_constants_impl(ctxt, const_resolver, None, values)
}

fn resolve_constants_impl<'a>(
    ctxt: &'a GcContext,
    const_resolver: &'a dyn ConstResolver,
    parent_layer: Option<&'a ValueLayer<'a>>,
    values: &'a [ConstValue],
) -> Result<Vec<Value>, RuntimeError> {
    type ResolverFn<'b> = Box<dyn FnOnce(&ValueLayer<'_>) -> Result<(), RuntimeError> + 'b>;
    let mut resolved_values = Vec::with_capacity(values.len());
    let mut resolvers: Vec<Option<ResolverFn<'a>>> = Vec::with_capacity(values.len());

    for value in values {
        let (value, resolver) = match value {
            ConstValue::ExternalRef(ConstIndex::Global(s)) => {
                (const_resolver.resolve_name(ctxt, s.as_str())?, None)
            }
            ConstValue::ExternalRef(ConstIndex::Local(index)) => {
                let value = parent_layer
                    .ok_or_else(|| {
                        RuntimeError::new_internal_error(
                            "Cannot resolve external ref without parent.",
                        )
                    })?
                    .get(index)?;
                (value, None)
            }
            ConstValue::Integer(i) => (Value::Integer(i.clone()), None),
            ConstValue::Float(f) => (Value::Float(f.clone()), None),
            ConstValue::String(s) => (Value::String(Rc::new(s.clone())), None),
            ConstValue::List(list) => {
                let (deferred, resolve_fn) = ctxt.create_deferred_ref();
                let resolver: ResolverFn<'a> = Box::new(move |vs| {
                    let mut list_elems = Vec::with_capacity(list.len());
                    for index in list {
                        let list_elem = match index {
                            ConstIndex::Local(index) => vs.get(index)?,
                            ConstIndex::Global(name) => const_resolver.resolve_name(ctxt, name)?,
                        };
                        list_elems.push(list_elem);
                    }
                    resolve_fn(List::from_iter(list_elems));
                    Ok(())
                });

                (Value::List(deferred), Some(resolver))
            }
            ConstValue::Function(const_func) => {
                let (deferred, resolve_fn) = ctxt.create_deferred_ref();
                let resolver: ResolverFn<'a> = Box::new(move |vs| {
                    let resolved_func_consts = resolve_constants_impl(
                        ctxt,
                        const_resolver,
                        Some(vs),
                        &const_func.const_table[..],
                    )?;
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

    let curr_value_layer = ValueLayer::new(parent_layer, &resolved_values);

    for resolver in resolvers.into_iter().flatten() {
        resolver(&curr_value_layer)?;
    }

    Ok(resolved_values)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NullResolver;

    impl ConstResolver for NullResolver {
        fn resolve_name(&self, _ctxt: &GcContext, _name: &str) -> Result<Value, RuntimeError> {
            Err(RuntimeError::new_internal_error("Unsupported"))
        }
    }

    #[test]
    fn build_simple_values() {
        let ctxt = GcContext::new();
        let const_resolver = &ctxt;
        let values = vec![
            ConstValue::Integer(Integer::Compact(42)),
            ConstValue::Float(Float::new(std::f64::consts::PI)),
            ConstValue::String("hello".to_string()),
        ];

        let resolved_values = resolve_constants(const_resolver, &NullResolver, &values).unwrap();
        assert_eq!(resolved_values.len(), 3);

        match &resolved_values[0] {
            Value::Integer(Integer::Compact(i)) => assert_eq!(*i, 42),
            _ => panic!("Expected integer value."),
        }

        match &resolved_values[1] {
            Value::Float(f) => assert_eq!(f.value(), std::f64::consts::PI),
            _ => panic!("Expected float value."),
        }

        match &resolved_values[2] {
            Value::String(s) => assert_eq!(s.as_str(), "hello"),
            _ => panic!("Expected string value."),
        }
    }

    #[test]
    fn build_composite_value() {
        let ctxt = GcContext::new();
        let const_resolver = &ctxt;
        let values = vec![
            ConstValue::Integer(Integer::Compact(42)),
            ConstValue::List(vec![
                ConstIndex::Local(LayerIndex::new_in_base(0)),
                ConstIndex::Local(LayerIndex::new_in_base(0)),
                ConstIndex::Local(LayerIndex::new_in_base(0)),
            ]),
        ];

        let resolved_values = resolve_constants(const_resolver, &NullResolver, &values).unwrap();
        assert_eq!(resolved_values.len(), 2);

        match &resolved_values[1] {
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
