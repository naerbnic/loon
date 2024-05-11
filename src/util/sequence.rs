pub trait Sequence<T> {
    fn collect<C>(self) -> C
    where
        C: FromIterator<T>;

    fn extend_into<C>(self, target: &mut C)
    where
        C: Extend<T>;
}

pub fn wrap_iter<I>(iter: I) -> impl Sequence<I::Item>
where
    I: Iterator,
{
    IterWrapper(iter)
}

struct IterWrapper<I>(I);

impl<I, T> Sequence<T> for IterWrapper<I>
where
    I: Iterator<Item = T>,
{
    fn collect<C>(self) -> C
    where
        C: FromIterator<T>,
    {
        self.0.collect()
    }

    fn extend_into<C>(self, target: &mut C)
    where
        C: Extend<T>,
    {
        target.extend(self.0);
    }
}
