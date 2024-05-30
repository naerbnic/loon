use std::cell::RefCell;

use crate::{
    gc::{GcRefVisitor, GcTraceable, PinnedGcRef},
    runtime::{
        error::{Result, RuntimeError},
        global_env::GlobalEnv,
        value::Value,
    },
};

use super::core::PinnedValue;

#[derive(Clone)]
pub struct List {
    items: RefCell<Vec<Value>>,
}

impl List {
    pub fn new(env: &GlobalEnv) -> PinnedGcRef<Self> {
        env.create_pinned_ref(List {
            items: RefCell::new(Vec::new()),
        })
    }

    pub fn from_iter(
        env: &GlobalEnv,
        iter: impl IntoIterator<Item = PinnedValue>,
    ) -> PinnedGcRef<Self> {
        let lock = env.lock_collect();
        env.create_pinned_ref(List {
            items: RefCell::new(iter.into_iter().map(|v| v.into_value(&lock)).collect()),
        })
    }

    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    pub fn at(&self, index: usize) -> PinnedValue {
        self.get(index).expect("Out of bounds list access")
    }

    pub fn get(&self, index: usize) -> Option<PinnedValue> {
        self.items.borrow().get(index).map(Value::pin)
    }

    pub fn append(&self, value: PinnedValue) {
        self.items.borrow_mut().push(value.to_value());
    }

    pub fn set(&self, index: u32, value: PinnedValue) -> Result<()> {
        let mut items = self.items.borrow_mut();
        *items
            .get_mut(index as usize)
            .ok_or_else(|| RuntimeError::new_internal_error("Index out of bounds."))? =
            value.to_value();
        Ok(())
    }
}

impl GcTraceable for List {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        let items = self.items.borrow();
        for item in &items[..] {
            item.trace(visitor);
        }
    }
}
