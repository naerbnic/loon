//! This module defines a simple garbage collector that uses a basic mark-and-sweep
//! algorithm.
//!
//! As a prototype, it is more important for the interface to be ergonomic,
//! rather than performant, The

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
};

use rc::{Rc, Weak};

mod rc;

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
struct PtrKey(*const ());

impl PtrKey {
    pub fn from_rc<T>(p: &Rc<RefCell<T>>) -> Self {
        PtrKey(Rc::as_ptr(p) as *const ())
    }

    pub fn from_weak<T>(p: &Weak<RefCell<T>>) -> Option<Self> {
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

struct ObjectInfoImpl<T>(Rc<RefCell<T>>);

impl<T> ObjectInfoImpl<T>
where
    T: GcTraceable,
{
    pub fn new(obj: Rc<RefCell<T>>) -> Self {
        Self(obj)
    }
}

impl<T> ObjectInfo for ObjectInfoImpl<T>
where
    T: GcTraceable,
{
    fn trace(&self, ptr_visitor: &mut dyn FnMut(PtrKey)) {
        GcTraceable::trace(&*self.0.borrow(), &mut PtrVisitor(ptr_visitor));
    }

    fn destroy(self: Box<Self>) {
        drop(self.0);
    }
}

struct ContextInner {
    live_objects: HashMap<PtrKey, Box<dyn ObjectInfo>>,
}

/// The main context object that manages a set of garbage collected objects.
///
/// This object is responsible for generating `Ref<T>` objects that are managed
/// by the garbage collector. Garbage collection happens only on demand
/// through the `garbage_collect()` method.
pub struct GcContext {
    inner: Rc<RefCell<ContextInner>>,
}

impl GcContext {
    /// Creates a new empty `GcContext`.
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(ContextInner {
                live_objects: HashMap::new(),
            })),
        }
    }

    fn downgrade(&self) -> WeakRefContext {
        WeakRefContext {
            inner: Rc::downgrade(&self.inner),
        }
    }

    fn accept_rc<T>(&self, obj: Rc<RefCell<T>>)
    where
        T: GcTraceable + 'static,
    {
        // We use the pointer as a key to the object in the HashMap.
        let ptr_id = PtrKey::from_rc(&obj);

        let obj_info = ObjectInfoImpl::new(obj);
        {
            let mut inner = self.inner.borrow_mut();
            inner.live_objects.insert(ptr_id, Box::new(obj_info));
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
        let deferred_obj = Weak::new();
        let obj = deferred_obj.clone();
        let weak_ctxt = self.downgrade();
        (GcRef { obj }, move |value| {
            let ctxt = match weak_ctxt.upgrade() {
                Some(ctxt) => ctxt,
                None => return,
            };
            let owned_obj = match deferred_obj.resurrect(RefCell::new(value)) {
                Ok(owned_obj) => owned_obj,
                Err(_) => panic!("object was already resolved"),
            };
            ctxt.accept_rc(owned_obj);
        })
    }

    /// Creates a new reference that is managed by the RefContext that contains
    /// the given value.
    pub fn create_ref<T>(&self, value: T) -> GcRef<T>
    where
        T: GcTraceable + 'static,
    {
        let owned_obj = Rc::new(RefCell::new(value));
        let obj = Rc::downgrade(&owned_obj);
        self.accept_rc(owned_obj);

        GcRef { obj }
    }

    pub fn garbage_collect(&self, roots: &GcRoots) {
        let mut inner = self.inner.borrow_mut();
        let mut reachable = HashSet::new();
        let mut worklist: VecDeque<_> = roots.roots.iter().cloned().collect();

        while let Some(ptr_id) = worklist.pop_front() {
            if reachable.insert(ptr_id) {
                if let Some(info) = inner.live_objects.get(&ptr_id) {
                    info.trace(&mut |key| {
                        if !reachable.contains(&key) {
                            worklist.push_back(key);
                        }
                    });
                }
            }
        }

        inner.live_objects.retain(|key, _| reachable.contains(key));
    }
}

impl Default for GcContext {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WeakRefContext {
    inner: Weak<RefCell<ContextInner>>,
}

impl WeakRefContext {
    pub fn upgrade(&self) -> Option<GcContext> {
        self.inner.upgrade().map(|inner| GcContext { inner })
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
    obj: Weak<RefCell<T>>,
}

impl<T> GcRef<T>
where
    T: GcTraceable + 'static,
{
    pub fn try_with_mut<F, R>(&self, body: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        let obj = self.obj.upgrade()?;
        let mut obj = obj.borrow_mut();
        Some(body(&mut *obj))
    }

    pub fn try_with<F, R>(&self, body: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        let obj = self.obj.upgrade()?;
        let obj = obj.borrow();
        Some(body(&*obj))
    }

    pub fn with_mut<F, R>(&self, body: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        self.try_with_mut(body)
            .expect("object was already destroyed")
    }

    pub fn with<F, R>(&self, body: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        self.try_with(body).expect("object was already destroyed")
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
        let ctxt = GcContext::new();
        let i_ref = ctxt.create_ref(4);
        let val = i_ref.with(|i| *i);
        assert_eq!(val, 4);
    }

    #[test]
    fn test_simple_gc() {
        let ctxt = GcContext::new();
        let i_ref = ctxt.create_ref(4);
        let mut roots = GcRoots::new();
        roots.add(&i_ref);
        ctxt.garbage_collect(&roots);
        let val = i_ref.with(|i| *i);
        assert_eq!(val, 4);
    }

    #[test]
    fn test_simple_gc_collect() {
        let ctxt = GcContext::new();
        let i_ref = ctxt.create_ref(4);
        ctxt.garbage_collect(&GcRoots::new());
        let val = i_ref.try_with_mut(|i| *i);
        assert!(val.is_none());
    }

    #[test]
    fn loop_collects() {
        let ctxt = GcContext::new();
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
