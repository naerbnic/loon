use std::cell::RefCell;

use crate::{
    gc::{GcRefVisitor, GcTraceable},
    runtime::{
        error::{Result, RuntimeError},
        value::Value,
    },
};

#[derive(Clone)]
pub struct List {
    items: RefCell<Vec<Value>>,
}

impl List {
    pub fn new() -> Self {
        List {
            items: RefCell::new(Vec::new()),
        }
    }

    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    pub fn at(&self, index: usize) -> Value {
        self.get(index).expect("Out of bounds list access").clone()
    }

    pub fn get(&self, index: usize) -> Option<Value> {
        self.items.borrow().get(index).cloned()
    }

    pub fn append(&self, value: Value) {
        self.items.borrow_mut().push(value);
    }

    pub fn set(&self, index: u32, value: Value) -> Result<()> {
        let mut items = self.items.borrow_mut();
        *items
            .get_mut(index as usize)
            .ok_or_else(|| RuntimeError::new_internal_error("Index out of bounds."))? = value;
        Ok(())
    }
}

impl FromIterator<Value> for List {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = Value>,
    {
        List {
            items: RefCell::new(iter.into_iter().collect()),
        }
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
