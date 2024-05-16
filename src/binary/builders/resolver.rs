pub trait ResolveOp<R, T, E>
where
    R: ?Sized,
{
    fn resolve_value(self: Box<Self>, resolver: &R) -> Result<T, E>;
}

impl<R, F, T, E> ResolveOp<R, T, E> for F
where
    R: ?Sized,
    F: for<'a> FnOnce(&R) -> Result<T, E>,
{
    fn resolve_value(self: Box<Self>, resolver: &R) -> Result<T, E> {
        self(resolver)
    }
}

pub struct ValueResolver<R, T, E>
where
    R: ?Sized,
{
    value_layer: Vec<Box<dyn ResolveOp<R, T, E>>>,
}

impl<R, T, E> ValueResolver<R, T, E>
where
    R: ?Sized,
{
    pub fn new() -> Self {
        ValueResolver {
            value_layer: Vec::new(),
        }
    }

    pub fn resolve_ref<F>(&mut self, op: F) -> usize
    where
        F: FnOnce(&R) -> Result<T, E> + 'static,
    {
        let new_value_index = self.value_layer.len();
        self.value_layer.push(Box::new(op));
        new_value_index
    }

    pub fn into_values(self, resolver: &R) -> Result<Vec<T>, E> {
        self.value_layer
            .into_iter()
            .map(|op| op.resolve_value(resolver))
            .collect::<Result<Vec<T>, E>>()
    }
}

impl<R, T, E> Default for ValueResolver<R, T, E>
where
    R: ?Sized,
{
    fn default() -> Self {
        Self::new()
    }
}
