//! Global contexts for the current state of a runtime environment.

use std::{borrow::Borrow, collections::HashMap, rc::Rc};

use super::value::Value;
use crate::refs::{GcContext, GcRef, GcTraceable};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GlobalSymbol(Rc<String>);

impl GlobalSymbol {
    pub fn new(symbol: impl Into<String>) -> Self {
        GlobalSymbol(Rc::new(symbol.into()))
    }
}

struct Inner {
    gc_context: GcContext,
    global_table: HashMap<GlobalSymbol, Value>,
}

#[derive(Clone)]
pub(crate) struct GlobalContext(Rc<Inner>);

impl GlobalContext {
    pub fn new() -> Self {
        GlobalContext(Rc::new(Inner {
            gc_context: GcContext::new(),
            global_table: HashMap::new(),
        }))
    }

    pub fn create_ref<T>(&self, value: T) -> GcRef<T>
    where
        T: GcTraceable + 'static,
    {
        self.0.gc_context.create_ref(value)
    }

    pub fn lookup_symbol(&self, symbol: &GlobalSymbol) -> Option<Value> {
        self.0.global_table.get(symbol).cloned()
    }
}

/// Crate internal methods for global context.
impl GlobalContext {
    pub(crate) fn create_deferred_ref<T>(&self) -> (GcRef<T>, impl FnOnce(T))
    where
        T: GcTraceable + 'static,
    {
        self.0.gc_context.create_deferred_ref()
    }
}
