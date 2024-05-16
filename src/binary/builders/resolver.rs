use super::disjoint_sets::{DisjointSet, SetIndex};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Conflict during resolution")]
    DisjointSet(#[from] super::disjoint_sets::Error),

    #[error("Unresolved Reference")]
    UnresolvedReference,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug)]
pub struct RefIndex(SetIndex);

#[derive(Clone, Copy, Debug)]
pub struct ValueIndex(usize);

impl ValueIndex {
    pub fn as_usize(self) -> usize {
        self.0
    }
}

pub struct RefResolver<'a> {
    index_layer: &'a DisjointSet<ValueIndex>,
}

impl RefResolver<'_> {
    pub fn resolve_ref(&self, index: RefIndex) -> Result<ValueIndex> {
        self.index_layer
            .find(index.0)
            .copied()
            .ok_or(Error::UnresolvedReference)
    }
}

pub trait ResolveOp<T> {
    fn resolve_value(self: Box<Self>, resolver: RefResolver<'_>) -> Result<T>;
}

impl<F, T> ResolveOp<T> for F
where
    F: for<'a> FnOnce(RefResolver<'a>) -> Result<T>,
{
    fn resolve_value(self: Box<Self>, resolver: RefResolver<'_>) -> Result<T> {
        self(resolver)
    }
}

pub struct ValueResolver<T> {
    index_layer: DisjointSet<ValueIndex>,
    value_layer: Vec<Box<dyn ResolveOp<T>>>,
}

impl<T> ValueResolver<T> {
    pub fn new() -> Self {
        ValueResolver {
            index_layer: DisjointSet::new(),
            value_layer: Vec::new(),
        }
    }

    pub fn new_value_ref(&mut self) -> RefIndex {
        RefIndex(self.index_layer.make_deferred_set())
    }

    pub fn resolve_to_other_ref(&mut self, from: RefIndex, to: RefIndex) -> Result<()> {
        Ok(self.index_layer.resolve_to_other_set(from.0, to.0)?)
    }

    pub fn resolve_ref<F>(&mut self, from: RefIndex, op: F) -> Result<ValueIndex>
    where
        F: FnOnce(RefResolver<'_>) -> Result<T> + 'static,
    {
        let new_value_index = ValueIndex(self.value_layer.len());
        self.index_layer.resolve_set(from.0, new_value_index)?;
        self.value_layer.push(Box::new(op));
        Ok(new_value_index)
    }

    pub fn get_value_index(&self, index: RefIndex) -> Result<ValueIndex> {
        self.index_layer
            .find(index.0)
            .copied()
            .ok_or(Error::UnresolvedReference)
    }

    pub fn is_index_resolved(&self, index: RefIndex) -> bool {
        self.index_layer.find(index.0).is_some()
    }

    pub fn into_values(self) -> Result<Vec<T>> {
        self.value_layer
            .into_iter()
            .map(|op| {
                op.resolve_value(RefResolver {
                    index_layer: &self.index_layer,
                })
            })
            .collect::<Result<Vec<T>>>()
    }
}

impl<T> Default for ValueResolver<T> {
    fn default() -> Self {
        Self::new()
    }
}
