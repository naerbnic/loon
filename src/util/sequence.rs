pub trait Sequence<T> {
    fn collect<C>(self) -> C
    where
        C: FromIterator<T>;

    fn extend_into<C>(self, target: &mut C)
    where
        C: Extend<T>;
}
