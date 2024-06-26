use std::{borrow::Borrow, hash::Hash};

pub struct InternSet<T> {
    map: std::collections::HashSet<T>,
}

impl<T> InternSet<T>
where
    T: Eq + Hash + Clone,
{
    pub fn new() -> Self {
        InternSet {
            map: std::collections::HashSet::new(),
        }
    }

    pub fn intern<V>(&mut self, value: &V) -> T
    where
        T: Borrow<V>,
        V: Hash + Eq + ?Sized,
        for<'a> &'a V: Into<T>,
    {
        if let Some(interned) = self.map.get(value) {
            interned.clone()
        } else {
            let value: T = value.into();
            self.map.insert(value.clone());
            value
        }
    }
}
