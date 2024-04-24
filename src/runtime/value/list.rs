use crate::{
    refs::{GcRefVisitor, GcTraceable},
    runtime::value::Value,
};

#[derive(Clone)]
pub struct List {
    items: Vec<Value>,
}

impl List {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn at(&self, index: usize) -> &Value {
        self.get(index).expect("Out of bounds list access")
    }

    pub fn get(&self, index: usize) -> Option<&Value> {
        self.items.get(index)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Value> {
        self.items.iter()
    }
}

impl FromIterator<Value> for List {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = Value>,
    {
        List {
            items: iter.into_iter().collect(),
        }
    }
}

impl GcTraceable for List {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        for item in &self.items {
            item.trace(visitor);
        }
    }
}
