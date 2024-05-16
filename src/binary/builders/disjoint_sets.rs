#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Resolved entry multiple times")]
    MultiplyResolved,
}

#[derive(Clone, Copy, Debug)]
pub struct SetIndex(usize);

enum Entry<T> {
    Root(T),
    Parent(SetIndex),
}

/// A way to represent disjoint sets, with a final value for resolution.
pub struct DisjointSet<T>(Vec<Option<Entry<T>>>);

impl<T> DisjointSet<T> {
    pub fn new() -> Self {
        DisjointSet(Vec::new())
    }

    // Generate a new set. The value is initially not yet resolved. Each set
    // may be resolved only once.
    pub fn make_deferred_set(&mut self) -> SetIndex {
        let index = SetIndex(self.0.len());
        self.0.push(None);
        index
    }

    pub fn resolve_set(&mut self, index: SetIndex, value: T) -> Result<(), Error> {
        if self.0[index.0].is_some() {
            return Err(Error::MultiplyResolved);
        }
        self.0[index.0] = Some(Entry::Root(value));
        Ok(())
    }

    pub fn resolve_to_other_set(&mut self, index: SetIndex, other: SetIndex) -> Result<(), Error> {
        if self.0[index.0].is_some() {
            return Err(Error::MultiplyResolved);
        }
        self.0[index.0] = Some(Entry::Parent(other));
        Ok(())
    }

    pub fn find(&self, index: SetIndex) -> Option<&T> {
        let mut current = index;
        loop {
            match self.0[current.0] {
                Some(Entry::Root(ref value)) => break Some(value),
                Some(Entry::Parent(next)) => current = next,
                None => break None,
            }
        }
    }
}
