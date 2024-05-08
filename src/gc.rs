//! This module defines a simple garbage collector that uses a basic mark-and-sweep
//! algorithm.
//!
//! As a prototype, it is more important for the interface to be ergonomic,
//! rather than performant, The

use std::{
    cell::{Cell, RefCell, UnsafeCell},
    collections::{HashMap, HashSet, VecDeque},
    mem::MaybeUninit,
};

use std::rc::{Rc, Weak};

struct InnerType<T> {
    /// A cell that is false if this object has not been resolved
    /// (where contents has not been initialized), and true if it has.
    is_resolved: Cell<bool>,
    contents: UnsafeCell<MaybeUninit<T>>,
}

impl<T> InnerType<T> {
    pub fn new(value: T) -> Self {
        Self {
            is_resolved: Cell::new(true),
            contents: UnsafeCell::new(MaybeUninit::new(value)),
        }
    }
    pub fn new_empty() -> Self {
        Self {
            is_resolved: Cell::new(false),
            contents: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
    pub fn is_resolved(&self) -> bool {
        self.is_resolved.get()
    }

    fn resolve_with(&self, value: T) -> Result<(), T> {
        if self.is_resolved.get() {
            Err(value)
        } else {
            // Safety: Since it has not been resolved, there must be no references
            // to the contents of the inner type.
            let cell_ref = unsafe { &mut *self.contents.get() };
            cell_ref.write(value);
            self.is_resolved.set(true);
            Ok(())
        }
    }

    fn try_as_ref(&self) -> Option<&T> {
        if self.is_resolved.get() {
            // Safety: Since it has been resolved, only other borrowed references
            // can exist.
            let uninit_cell = unsafe { &*self.contents.get() };

            // Safety: Since it has been resolved, the value is initialized.
            let resolved_ref = unsafe { uninit_cell.assume_init_ref() };
            Some(resolved_ref)
        } else {
            None
        }
    }
}

impl<T> Drop for InnerType<T> {
    fn drop(&mut self) {
        if self.is_resolved.get() {
            // Safety: Since it has been resolved, only other borrowed references
            // can exist.
            let uninit_cell = unsafe { &mut *self.contents.get() };

            // Safety: Since it has been resolved, the value is initialized.
            unsafe { uninit_cell.assume_init_drop() };
        }
    }
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
struct PtrKey(*const ());

impl PtrKey {
    pub fn from_rc<T>(p: &Rc<InnerType<T>>) -> Self {
        PtrKey(Rc::as_ptr(p) as *const ())
    }

    pub fn from_weak<T>(p: &Weak<InnerType<T>>) -> Option<Self> {
        Some(PtrKey::from_rc(&p.upgrade()?))
    }
}

trait ObjectInfo {
    fn trace(&self, ptr_visitor: &mut dyn FnMut(PtrKey));
    fn destroy(self: Box<Self>);
}

struct PtrVisitor<'a>(&'a mut dyn FnMut(PtrKey));

impl GcRefVisitor for PtrVisitor<'_> {
    fn visit<T>(&mut self, obj: &GcRef<T>)
    where
        T: GcTraceable + 'static,
    {
        if let Some(key) = PtrKey::from_weak(&obj.obj) {
            (self.0)(key);
        }
    }
}

struct RootsVisitor<'a>(&'a mut GcRoots);

impl GcRefVisitor for RootsVisitor<'_> {
    fn visit<T>(&mut self, obj: &GcRef<T>)
    where
        T: GcTraceable + 'static,
    {
        self.0.add(obj);
    }
}

struct ObjectInfoImpl<T>(Rc<InnerType<T>>);

impl<T> ObjectInfoImpl<T>
where
    T: GcTraceable,
{
    pub fn new(obj: Rc<InnerType<T>>) -> Self {
        Self(obj)
    }
}

impl<T> ObjectInfo for ObjectInfoImpl<T>
where
    T: GcTraceable,
{
    fn trace(&self, ptr_visitor: &mut dyn FnMut(PtrKey)) {
        if let Some(obj) = self.0.try_as_ref() {
            obj.trace(&mut PtrVisitor(ptr_visitor));
        }
    }

    fn destroy(self: Box<Self>) {
        drop(self.0);
    }
}

type RootGatherer = Box<dyn Fn(&mut GcRoots)>;

struct EnvInner {
    live_objects: RefCell<HashMap<PtrKey, Box<dyn ObjectInfo>>>,
    root_gatherer: Option<RootGatherer>,
    alloc_count: Cell<usize>,
    alloc_count_limit: usize,
}

/// The main context object that manages a set of garbage collected objects.
///
/// This object is responsible for generating `Ref<T>` objects that are managed
/// by the garbage collector. Garbage collection happens only on demand
/// through the `garbage_collect()` method.
pub struct GcEnv {
    inner: Rc<EnvInner>,
}

impl GcEnv {
    const DEFAULT_ALLOC_COUNT_LIMIT: usize = 100;
    /// Creates a new empty `GcContext`.
    pub fn new() -> Self {
        Self {
            inner: Rc::new(EnvInner {
                live_objects: RefCell::new(HashMap::new()),
                root_gatherer: None,
                alloc_count: Cell::new(0),
                alloc_count_limit: Self::DEFAULT_ALLOC_COUNT_LIMIT,
            }),
        }
    }

    pub fn with_root_gatherer<F>(alloc_limit: usize, gatherer: F) -> Self
    where
        F: Fn(&mut GcRoots) + 'static,
    {
        Self {
            inner: Rc::new(EnvInner {
                live_objects: RefCell::new(HashMap::new()),
                root_gatherer: Some(Box::new(gatherer)),
                alloc_count: Cell::new(0),
                alloc_count_limit: alloc_limit,
            }),
        }
    }

    fn downgrade(&self) -> WeakRefContext {
        WeakRefContext {
            inner: Rc::downgrade(&self.inner),
        }
    }

    fn accept_rc<T>(&self, obj: Rc<InnerType<T>>)
    where
        T: GcTraceable + 'static,
    {
        if let Some(gatherer) = &self.inner.root_gatherer {
            if self.inner.alloc_count.get() >= self.inner.alloc_count_limit {
                let mut roots = GcRoots::new();
                gatherer(&mut roots);
                self.garbage_collect(&roots);
                self.inner.alloc_count.set(0);
            }
        }

        // We use the pointer as a key to the object in the HashMap.
        let ptr_id = PtrKey::from_rc(&obj);

        let obj_info = ObjectInfoImpl::new(obj);
        {
            let mut live_objects = self.inner.live_objects.borrow_mut();
            live_objects.insert(ptr_id, Box::new(obj_info));
        }
    }

    /// Creates a new reference that will be managed by the RefContext, but
    /// not yet resolved. `Ref<T>` objects created by this method will not
    /// have any value associated with them until the deferred reference is
    /// resolved.
    ///
    /// To resolve the reference, the function returned by this method must be
    /// called with a value. References will then be updated to point to the
    /// new value.
    pub fn create_deferred_ref<T>(&self) -> (GcRef<T>, impl FnOnce(T))
    where
        T: GcTraceable + 'static,
    {
        // We create a weakref that we can resurrect when needed.
        let deferred_obj = Rc::new(InnerType::new_empty());
        let obj = Rc::downgrade(&deferred_obj);
        let weak_ctxt = self.downgrade();
        (GcRef { obj }, move |value| {
            let Some(ctxt) = weak_ctxt.upgrade() else {
                return;
            };

            {
                let result = deferred_obj.resolve_with(value);
                if result.is_err() {
                    panic!("object was already resolved");
                }
            }
            ctxt.accept_rc(deferred_obj);
        })
    }

    /// Creates a new reference that is managed by the RefContext that contains
    /// the given value.
    pub fn create_ref<T>(&self, value: T) -> GcRef<T>
    where
        T: GcTraceable + 'static,
    {
        let owned_obj = Rc::new(InnerType::new(value));
        let obj = Rc::downgrade(&owned_obj);
        self.accept_rc(owned_obj);

        GcRef { obj }
    }

    pub fn garbage_collect(&self, roots: &GcRoots) {
        let mut live_objects = self.inner.live_objects.borrow_mut();
        let mut reachable = HashSet::new();
        let mut worklist: VecDeque<_> = roots.roots.iter().cloned().collect();

        while let Some(ptr_id) = worklist.pop_front() {
            if reachable.insert(ptr_id) {
                if let Some(info) = live_objects.get(&ptr_id) {
                    info.trace(&mut |key| {
                        if !reachable.contains(&key) {
                            worklist.push_back(key);
                        }
                    });
                }
            }
        }

        live_objects.retain(|key, _| reachable.contains(key));
    }
}

impl Default for GcEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// A reference to a garbage collected object.
///
/// To preserve safety, we do not allow direct access to the object. Instead,
/// the object must be accessed through the `with` methods.
pub struct GcRef<T>
where
    T: GcTraceable + 'static,
{
    obj: Weak<InnerType<T>>,
}

impl<T> GcRef<T>
where
    T: GcTraceable + 'static,
{
    pub fn try_borrow(&self) -> Option<GcRefGuard<T>> {
        let obj = self.obj.upgrade()?;
        if !obj.is_resolved() {
            return None;
        }
        Some(GcRefGuard {
            obj,
            _phantom: std::marker::PhantomData,
        })
    }

    pub fn borrow(&self) -> GcRefGuard<T> {
        self.try_borrow().expect("object was not resolved")
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.obj, &other.obj)
    }
}

impl<T> Clone for GcRef<T>
where
    T: GcTraceable + 'static,
{
    fn clone(&self) -> Self {
        Self {
            obj: self.obj.clone(),
        }
    }
}

impl<T> GcTraceable for GcRef<T>
where
    T: GcTraceable + 'static,
{
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        visitor.visit(self);
    }
}

pub struct GcRefGuard<'a, T>
where
    T: GcTraceable + 'static,
{
    obj: Rc<InnerType<T>>,
    _phantom: std::marker::PhantomData<&'a T>,
}

impl<T> std::ops::Deref for GcRefGuard<'_, T>
where
    T: GcTraceable + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.obj.try_as_ref().expect("object was still valid")
    }
}

pub struct WeakRefContext {
    inner: Weak<EnvInner>,
}

impl WeakRefContext {
    pub fn upgrade(&self) -> Option<GcEnv> {
        self.inner.upgrade().map(|inner| GcEnv { inner })
    }
}

/// An object that is used to collect a collection of references that are
/// considered roots for a single garbage collection pass.
pub struct GcRoots {
    roots: HashSet<PtrKey>,
}

impl GcRoots {
    /// Creates a new empty GcRoots object.
    pub fn new() -> Self {
        Self {
            roots: HashSet::new(),
        }
    }

    /// Add the given reference to the roots collection.
    pub fn add<T>(&mut self, obj: &GcRef<T>)
    where
        T: GcTraceable + 'static,
    {
        if let Some(key) = PtrKey::from_weak(&obj.obj) {
            self.roots.insert(key);
        }
    }

    pub fn visit<T>(&mut self, obj: &T)
    where
        T: GcTraceable + 'static,
    {
        obj.trace(&mut RootsVisitor(self));
    }
}

impl Default for GcRoots {
    fn default() -> Self {
        Self::new()
    }
}

/// A trait that allows an object to be visited by a GcRefVisitor.
pub trait GcRefVisitor {
    /// Visits the given reference.
    fn visit<T>(&mut self, obj: &GcRef<T>)
    where
        T: GcTraceable + 'static;
}

/// A trait that allows an object to be traced by the garbage collector.
///
/// All objcets that are managed by the garbage collector must implement this.
pub trait GcTraceable {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor;
}

macro_rules! impl_primitive_gc {
    ($($t:ty),*) => {
        $(
            impl GcTraceable for $t {
                fn trace<V>(&self, _visitor: &mut V)
                where
                    V: GcRefVisitor,
                {
                    // No nested values to trace
                }
            }
        )*
    };
}

impl_primitive_gc!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);
impl_primitive_gc!(bool, char);
impl_primitive_gc!(String);

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    macro_rules! gc_roots {
        ($($e:expr),*) => {
            {
                #[allow(unused_mut)]
                let mut roots = GcRoots::new();
                $(roots.add($e);)*
                roots
            }
        };
    }

    struct Node {
        children: RefCell<Vec<GcRef<Node>>>,
        drop_cell: Rc<Cell<bool>>,
    }

    impl Node {
        fn new() -> (Self, impl Fn() -> bool) {
            let drop_cell = Rc::new(Cell::new(false));
            (
                Self {
                    children: RefCell::new(Vec::new()),
                    drop_cell: drop_cell.clone(),
                },
                move || drop_cell.get(),
            )
        }

        fn add_child(&self, node: GcRef<Node>) {
            self.children.borrow_mut().push(node);
        }
    }

    impl GcTraceable for Node {
        fn trace<V>(&self, visitor: &mut V)
        where
            V: GcRefVisitor,
        {
            let children = self.children.borrow();
            for child in children.iter() {
                visitor.visit(child);
            }
        }
    }

    impl Drop for Node {
        fn drop(&mut self) {
            self.drop_cell.set(true);
        }
    }

    #[test]
    fn test_ref_works() {
        let ctxt = GcEnv::new();
        let i_ref = ctxt.create_ref(4);
        let val = *i_ref.borrow();
        assert_eq!(val, 4);
    }

    #[test]
    fn test_simple_gc() {
        let ctxt = GcEnv::new();
        let i_ref = ctxt.create_ref(4);
        let mut roots = GcRoots::new();
        roots.add(&i_ref);
        ctxt.garbage_collect(&roots);
        let val = *i_ref.borrow();
        assert_eq!(val, 4);
    }

    #[test]
    fn test_simple_gc_collect() {
        let ctxt = GcEnv::new();
        let i_ref = ctxt.create_ref(4);
        ctxt.garbage_collect(&GcRoots::new());
        let val = i_ref.try_borrow();
        assert!(val.is_none());
    }

    #[test]
    fn loop_collects() {
        let ctxt = GcEnv::new();
        let (node1, drop1) = Node::new();
        let (node2, drop2) = Node::new();
        let (node2_ref, resolve_node2_ref) = ctxt.create_deferred_ref();
        node1.add_child(node2_ref);
        let node1_ref = ctxt.create_ref(node1);
        node2.add_child(node1_ref.clone());
        resolve_node2_ref(node2);
        assert!(!drop1());
        assert!(!drop2());

        // With either of the two, both should not be collected.
        ctxt.garbage_collect(&gc_roots!(&node1_ref));
        assert!(!drop1());
        assert!(!drop2());

        ctxt.garbage_collect(&gc_roots!());
        assert!(drop1());
        assert!(drop2());
    }
}
