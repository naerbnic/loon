#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Conflict during resolution")]
    ResolveConflict,

    #[error("Unresolved Reference")]
    UnresolvedReference,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RefIndex(usize);
#[derive(Clone, Copy, Debug)]
pub struct ValueIndex(usize);

impl ValueIndex {
    pub fn as_usize(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug)]
enum Index {
    UnfiedWith(RefIndex),
    Value(ValueIndex),
}

pub struct RefResolver<'a> {
    index_layer: &'a Vec<Option<Index>>,
}

impl RefResolver<'_> {
    pub fn resolve_ref(&self, index: RefIndex) -> Result<ValueIndex> {
        let mut result = index;
        while let Some(Index::UnfiedWith(next)) = self.index_layer[result.0] {
            result = next;
        }
        match self.index_layer[result.0] {
            Some(Index::Value(value_index)) => Ok(value_index),
            _ => Err(Error::UnresolvedReference),
        }
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
    index_layer: Vec<Option<Index>>,
    value_layer: Vec<Box<dyn ResolveOp<T>>>,
}

impl<T> ValueResolver<T> {
    pub fn new() -> Self {
        ValueResolver {
            index_layer: Vec::new(),
            value_layer: Vec::new(),
        }
    }

    pub fn new_value_ref(&mut self) -> RefIndex {
        let index = RefIndex(self.index_layer.len());
        self.index_layer.push(None);
        index
    }

    pub fn unify_refs(&mut self, a: RefIndex, b: RefIndex) -> Result<()> {
        let resolved_a = self.resolve_index(a);
        let resolved_b = self.resolve_index(b);

        // If both values are already in the same set, we're done.
        if resolved_a == resolved_b {
            return Ok(());
        }

        // If one of the values is resolved, the other has to be set to a cross
        // reference.
        let (from, to) = if self.index_layer[resolved_a.0].is_some() {
            (resolved_b, resolved_a)
        } else {
            (resolved_a, resolved_b)
        };

        if self.index_layer[from.0].is_some() {
            return Err(Error::ResolveConflict);
        }
        self.index_layer[from.0] = Some(Index::UnfiedWith(to));
        Ok(())
    }

    pub fn resolve_ref<F>(&mut self, from: RefIndex, op: F) -> Result<ValueIndex>
    where
        F: FnOnce(RefResolver<'_>) -> Result<T> + 'static,
    {
        let resolved_from = self.resolve_index(from);

        if self.index_layer[resolved_from.0].is_some() {
            return Err(Error::ResolveConflict);
        }

        let value_index = ValueIndex(self.value_layer.len());
        self.index_layer[resolved_from.0] = Some(Index::Value(value_index));
        self.value_layer.push(Box::new(op));
        Ok(value_index)
    }

    pub fn get_value_index(&self, index: RefIndex) -> Result<ValueIndex> {
        let resolved_index = self.resolve_index(index);
        match self.index_layer[resolved_index.0] {
            Some(Index::Value(value_index)) => Ok(value_index),
            _ => Err(Error::ResolveConflict),
        }
    }

    pub fn is_index_resolved(&self, index: RefIndex) -> bool {
        self.index_layer[index.0].is_some()
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

    fn resolve_index(&self, index: RefIndex) -> RefIndex {
        let mut result = index;
        while let Some(Index::UnfiedWith(next)) = self.index_layer[result.0] {
            result = next;
        }
        result
    }
}

impl<T> Default for ValueResolver<T> {
    fn default() -> Self {
        Self::new()
    }
}
