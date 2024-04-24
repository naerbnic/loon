//! Loon has constants that represent constant values that can be resolved at
//! runtime. They don't themselves refer to Values, as that would require the
//! presence of a runtime, but they can be used to create Values.

use std::rc::Rc;

use crate::{refs::GcContext, runtime::value::List};

use super::{
    error::RuntimeError,
    instructions::InstructionList,
    value::{Float, Integer, Value},
};

#[derive(Clone, Debug)]
pub enum ConstIndex {
    /// An index into the local constant table.
    Local(usize),

    /// An index to be resolved globally by name.
    Global(Rc<String>),
}

#[derive(Clone, Debug)]
pub struct ConstFunction {
    const_table: Vec<ConstIndex>,
    instructions: Rc<InstructionList>,
}

#[derive(Clone, Debug)]
pub enum ConstValue {
    Integer(Integer),
    Float(Float),
    String(String),
    List(Vec<ConstIndex>),
    Function(ConstFunction),
}

pub trait ConstResolver {
    fn resolve_name(&self, ctxt: &GcContext, name: &str) -> Result<Value, RuntimeError>;
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
    type ResolverFn<'b> = Box<dyn FnOnce(&[Value]) -> Result<(), RuntimeError> + 'b>;
    let mut resolved_values = Vec::with_capacity(values.len());
    let mut resolvers: Vec<Option<ResolverFn<'a>>> = Vec::with_capacity(values.len());

    for value in values {
        let (value, resolver) = match value {
            ConstValue::Integer(i) => (Value::Integer(i.clone()), None),
            ConstValue::Float(f) => (Value::Float(f.clone()), None),
            ConstValue::String(s) => (Value::String(Rc::new(s.clone())), None),
            ConstValue::List(list) => {
                let (deferred, resolve_fn) = ctxt.create_deferred_ref();
                let resolver: ResolverFn<'a> = Box::new(move |vs| {
                    let mut list_elems = Vec::with_capacity(list.len());
                    for index in list {
                        let list_elem = match index {
                            ConstIndex::Local(index) => vs[*index].clone(),
                            ConstIndex::Global(name) => const_resolver.resolve_name(ctxt, name)?,
                        };
                        list_elems.push(list_elem);
                    }
                    resolve_fn(List::from_iter(list_elems));
                    Ok(())
                });

                (Value::List(deferred), Some(resolver))
            }
            ConstValue::Function(_) => todo!(),
        };
        resolved_values.push(value);
        resolvers.push(resolver);
    }

    for resolver in resolvers.into_iter().flatten() {
        resolver(&resolved_values)?;
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
                ConstIndex::Local(0),
                ConstIndex::Local(0),
                ConstIndex::Local(0),
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
