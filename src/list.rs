use crate::{
    refs::{GcRefVisitor, GcTraceable},
    Value,
};

#[derive(Clone)]
pub struct List {
    items: Vec<Value>,
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
