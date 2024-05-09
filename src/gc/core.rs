use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
};

use std::rc::{Rc, Weak};

struct InnerType<T> {
    ref_count: Cell<usize>,
    pin_count: Cell<usize>,
    contents: OnceCell<T>,
}

impl<T> InnerType<T> {
    pub fn new(value: T) -> Self {
        Self {
            ref_count: Cell::new(0),
            pin_count: Cell::new(0),
            contents: value.into(),
        }
    }
    pub fn new_empty() -> Self {
        Self {
            ref_count: Cell::new(0),
            pin_count: Cell::new(0),
            contents: OnceCell::new(),
        }
    }
    pub fn is_resolved(&self) -> bool {
        self.contents.get().is_some()
    }

    fn resolve_with(&self, value: T) -> Result<(), T> {
        self.contents.set(value)
    }

    fn try_as_ref(&self) -> Option<&T> {
        self.contents.get()
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
    fn is_pinned(&self) -> bool;
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
    fn is_pinned(&self) -> bool {
        self.0.pin_count.get() > 0
    }

    fn trace(&self, ptr_visitor: &mut dyn FnMut(PtrKey)) {
        if let Some(obj) = self.0.try_as_ref() {
            obj.trace(&mut PtrVisitor(ptr_visitor));
        }
    }

    fn destroy(self: Box<Self>) {
        drop(self.0);
    }
}

thread_local! {
    static CURR_ENV: RefCell<Option<ControlPtr>> = const { RefCell::new(None) };
}

struct ControlData {
    live_objects: RefCell<HashMap<PtrKey, Box<dyn ObjectInfo>>>,
    alloc_count: Cell<usize>,
    alloc_count_limit: usize,
}

#[derive(Clone)]
struct ControlPtr {
    control: Rc<ControlData>,
}

impl ControlPtr {
    /// Creates a new empty `GcContext`.
    pub fn new(alloc_limit: usize) -> Self {
        Self {
            control: Rc::new(ControlData {
                live_objects: RefCell::new(HashMap::new()),
                alloc_count: Cell::new(0),
                alloc_count_limit: alloc_limit,
            }),
        }
    }

    pub fn downgrade(&self) -> WeakRefContext {
        WeakRefContext {
            inner: Rc::downgrade(&self.control),
        }
    }

    pub fn accept_rc<T>(&self, obj: Rc<InnerType<T>>)
    where
        T: GcTraceable + 'static,
    {
        if self.control.alloc_count.get() >= self.control.alloc_count_limit {
            self.garbage_collect();
            self.control.alloc_count.set(0);
        }

        // We use the pointer as a key to the object in the HashMap.
        let ptr_id = PtrKey::from_rc(&obj);

        let obj_info = ObjectInfoImpl::new(obj);
        {
            let mut live_objects = self.control.live_objects.borrow_mut();
            live_objects.insert(ptr_id, Box::new(obj_info));
        }
    }

    /// Creates a new reference that is managed by the RefContext that contains
    /// the given value.
    pub fn create_ref<T>(&self, value: T) -> GcRef<T>
    where
        T: GcTraceable + 'static,
    {
        let owned_obj = Rc::new(InnerType::new(value));
        let obj = owned_obj.clone();
        self.accept_rc(owned_obj);

        GcRef::from_rc(obj)
    }

    pub fn garbage_collect(&self) {
        let mut live_objects = self.control.live_objects.borrow_mut();
        let mut reachable = HashSet::new();
        let mut worklist: VecDeque<_> = live_objects
            .iter()
            .filter_map(|(k, v)| if v.is_pinned() { Some(*k) } else { None })
            .collect();

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

/// The main context object that manages a set of garbage collected objects.
///
/// This object is responsible for generating `Ref<T>` objects that are managed
/// by the garbage collector. Garbage collection happens only on demand
/// through the `garbage_collect()` method.
pub struct GcEnv(ControlPtr);

impl GcEnv {
    pub fn new(alloc_limit: usize) -> Self {
        Self(ControlPtr::new(alloc_limit))
    }

    pub fn with<F, R>(&self, body: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _borrow = self.borrow();

        body()
    }

    pub fn borrow(&self) -> GcEnvGuard {
        CURR_ENV.with(|env| {
            let prev_env = env.borrow_mut().replace(self.0.clone());
            if prev_env.is_some() {
                panic!("nested GcEnv::with() calls are not allowed");
            }
        });
        GcEnvGuard { env: self }
    }
}

pub struct GcEnvGuard<'a> {
    env: &'a GcEnv,
}

impl Drop for GcEnvGuard<'_> {
    fn drop(&mut self) {
        CURR_ENV.with(|env| {
            let prev_env = env
                .borrow_mut()
                .take()
                .expect("GcEnv::with() was not called");
            assert!(Rc::ptr_eq(&prev_env.control, &self.env.0.control));
        });
    }
}

pub fn create_ref<T>(value: T) -> GcRef<T>
where
    T: GcTraceable + 'static,
{
    CURR_ENV.with(|env| {
        let env = env.borrow();
        env.as_ref()
            .expect("Not in thread scope of a GcEnv::with() call")
            .create_ref(value)
    })
}

#[cfg(test)]
pub(super) fn garbage_collect() {
    CURR_ENV.with(|env| {
        let env = env.borrow();
        env.as_ref()
            .expect("Not in thread scope of a GcEnv::with() call")
            .garbage_collect();
    })
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
    fn from_rc(obj: Rc<InnerType<T>>) -> Self {
        obj.ref_count.set(obj.ref_count.get() + 1);
        Self {
            obj: Rc::downgrade(&obj),
        }
    }

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

    pub fn try_pin(&self) -> Option<PinnedGcRef<T>> {
        Some(PinnedGcRef::from_rc(self.obj.upgrade()?))
    }

    pub fn pin(&self) -> PinnedGcRef<T> {
        PinnedGcRef::from_rc(self.obj.upgrade().expect("object was not resolved"))
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
        if let Some(obj) = self.obj.upgrade() {
            obj.ref_count.set(obj.ref_count.get() + 1);
        }
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

impl<T> Drop for GcRef<T>
where
    T: GcTraceable + 'static,
{
    fn drop(&mut self) {
        if let Some(obj) = self.obj.upgrade() {
            obj.ref_count.set(obj.ref_count.get() - 1);
        }
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

pub struct PinnedGcRef<T> {
    obj: Rc<InnerType<T>>,
}

impl<T> PinnedGcRef<T>
where
    T: GcTraceable + 'static,
{
    /// Private method to convert a `GcRef` into a `PinnedGcRef`.
    fn from_rc(obj: Rc<InnerType<T>>) -> Self {
        obj.pin_count.set(obj.pin_count.get() + 1);
        Self { obj }
    }

    pub fn try_borrow(&self) -> Option<GcRefGuard<T>> {
        if !self.obj.is_resolved() {
            return None;
        }
        Some(GcRefGuard {
            obj: self.obj.clone(),
            _phantom: std::marker::PhantomData,
        })
    }

    pub fn borrow(&self) -> GcRefGuard<T> {
        self.try_borrow().expect("object was not resolved")
    }
}

impl<T> Drop for PinnedGcRef<T> {
    fn drop(&mut self) {
        self.obj.pin_count.set(self.obj.pin_count.get() - 1);
    }
}

struct WeakRefContext {
    inner: Weak<ControlData>,
}

impl WeakRefContext {
    pub fn upgrade(&self) -> Option<ControlPtr> {
        self.inner
            .upgrade()
            .map(|inner| ControlPtr { control: inner })
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

trait PinnedObject {
    fn collect_ptrs(&self, visitor: &mut dyn FnMut(PtrKey));
}

pub struct PinnedObjectWrapper<T>(T);

impl<T> PinnedObject for PinnedObjectWrapper<T>
where
    T: GcTraceable,
{
    fn collect_ptrs(&self, visitor: &mut dyn FnMut(PtrKey)) {
        self.0.trace(&mut PtrVisitor(visitor));
    }
}
