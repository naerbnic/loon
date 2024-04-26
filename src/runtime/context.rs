//! Global contexts for the current state of a runtime environment.

use std::{
    cell::RefCell,
    collections::{hash_map, HashMap},
    rc::Rc,
};

use super::{error::RuntimeError, instructions::InstEvalList, value::Value};
use crate::{
    binary::{instructions::InstructionList, symbols::GlobalSymbol},
    refs::{GcContext, GcRef, GcTraceable},
};

struct Inner {
    gc_context: GcContext,
    global_table: RefCell<HashMap<GlobalSymbol, Value>>,
}

#[derive(Clone)]
pub(crate) struct GlobalContext(Rc<Inner>);

impl GlobalContext {
    pub fn new() -> Self {
        GlobalContext(Rc::new(Inner {
            gc_context: GcContext::new(),
            global_table: RefCell::new(HashMap::new()),
        }))
    }

    pub fn create_ref<T>(&self, value: T) -> GcRef<T>
    where
        T: GcTraceable + 'static,
    {
        self.0.gc_context.create_ref(value)
    }

    pub fn lookup_symbol(&self, symbol: &GlobalSymbol) -> Option<Value> {
        self.0.global_table.borrow().get(symbol).cloned()
    }

    pub fn resolve_instructions(
        &self,
        inst_list: &InstructionList,
    ) -> Result<InstEvalList, RuntimeError> {
        todo!()
    }

    pub fn insert_symbols(
        &self,
        symbols: impl IntoIterator<Item = (GlobalSymbol, Value)>,
    ) -> Result<(), RuntimeError> {
        let mut table_mut = self.0.global_table.borrow_mut();
        for (sym, value) in symbols {
            match table_mut.entry(sym) {
                hash_map::Entry::Occupied(_) => {
                    return Err(RuntimeError::new_internal_error("Symbol already defined."))
                }
                hash_map::Entry::Vacant(vac) => {
                    vac.insert(value);
                }
            }
        }
        Ok(())
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
