use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
};

use std::rc::{Rc, Weak};

use super::counter::Counter;

struct InnerType<T>
where
    T: ?Sized,
{
    ref_count: Counter,
    pin_count: Counter,
    contents: T,
}

impl<T> InnerType<T> {
    pub fn new(contents: T) -> Self {
        Self {
            ref_count: Counter::new(),
            pin_count: Counter::new(),
            contents,
        }
    }

    fn as_ref(&self) -> &T {
        &self.contents
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
        self.0.pin_count.is_nonzero()
    }

    fn trace(&self, ptr_visitor: &mut dyn FnMut(PtrKey)) {
        (*self.0).as_ref().trace(&mut PtrVisitor(ptr_visitor));
    }
}

struct ControlData {
    live_objects: RefCell<HashMap<PtrKey, Box<dyn ObjectInfo>>>,
    collect_guard_count: Counter,
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
                collect_guard_count: Counter::new(),
                alloc_count: Cell::new(0),
                alloc_count_limit: alloc_limit,
            }),
        }
    }

    pub fn accept_rc<T>(&self, obj: Rc<InnerType<T>>)
    where
        T: GcTraceable + 'static,
    {
        self.control
            .alloc_count
            .set(self.control.alloc_count.get() + 1);
        self.attempt_garbage_collect();

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

    pub fn attempt_garbage_collect(&self) {
        if self.control.collect_guard_count.is_zero()
            && self.control.alloc_count.get() >= self.control.alloc_count_limit
        {
            self.garbage_collect();
            self.control.alloc_count.set(0);
        }
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
#[derive(Clone)]
pub struct GcEnv(ControlPtr);

impl GcEnv {
    pub fn new(alloc_limit: usize) -> Self {
        Self(ControlPtr::new(alloc_limit))
    }

    pub fn lock_collect(&self) -> CollectGuard {
        CollectGuard::new(&self.0)
    }

    pub fn create_pinned_ref<T>(&self, value: T) -> PinnedGcRef<T>
    where
        T: GcTraceable + 'static,
    {
        let collect_guard = self.lock_collect();
        collect_guard.create_ref(value).pin()
    }

    #[cfg(test)]
    pub fn force_collect(&self) {
        self.0.garbage_collect();
    }
}

/// A guard on a [`GcEnv`] that ensures that no garbage collections happen
/// during the dynamic scope of this object. Any non-pinned [`GcRef`] values
/// that are not reachable from [`PinnedGcRef`] roots must be manipulated
/// behind this guard.
pub struct CollectGuard<'a>(&'a ControlPtr);

impl<'a> CollectGuard<'a> {
    fn new(control_ptr: &'a ControlPtr) -> Self {
        control_ptr.control.collect_guard_count.increment();
        Self(control_ptr)
    }

    /// Create a GcRef within this environment, wrapping the given
    /// value. A value needs to be tracable.
    ///
    /// This is built through a CollectGuard in order to ensure that
    /// the returned reference isn't immediately possible to be collected.
    pub fn create_ref<T>(&self, value: T) -> GcRef<T>
    where
        T: GcTraceable + 'static,
    {
        self.0.create_ref(value)
    }
}

impl<'a> Clone for CollectGuard<'a> {
    fn clone(&self) -> Self {
        self.0.control.collect_guard_count.increment();
        Self(self.0)
    }
}

impl<'a> Drop for CollectGuard<'a> {
    fn drop(&mut self) {
        self.0.control.collect_guard_count.decrement();
        if self.0.control.collect_guard_count.is_zero() {
            self.0.attempt_garbage_collect();
        }
    }
}

/// A reference to a garbage collected object.
///
/// To preserve safety, we do not allow direct access to the object. Instead,
/// the object must be accessed through the `with` methods.
pub struct GcRef<T>
where
    T: ?Sized + 'static,
{
    obj: Weak<InnerType<T>>,
}

impl<T> GcRef<T>
where
    T: ?Sized + 'static,
{
    fn from_rc(obj: Rc<InnerType<T>>) -> Self {
        obj.ref_count.increment();
        Self {
            obj: Rc::downgrade(&obj),
        }
    }

    pub fn try_borrow(&self) -> Option<GcRefGuard<T>> {
        let obj = self.obj.upgrade()?;
        Some(GcRefGuard {
            obj,
            _phantom: std::marker::PhantomData,
        })
    }

    pub fn borrow(&self) -> GcRefGuard<T> {
        self.try_borrow().expect("object was deleted")
    }

    pub fn into_pinned(self) -> PinnedGcRef<T> {
        self.pin()
    }

    pub fn pin(&self) -> PinnedGcRef<T> {
        PinnedGcRef::from_rc(self.obj.upgrade().expect("object was deleted"))
    }

    pub fn ref_eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.obj, &other.obj)
    }
}

impl<T> Clone for GcRef<T>
where
    T: GcTraceable + 'static,
{
    fn clone(&self) -> Self {
        if let Some(obj) = self.obj.upgrade() {
            obj.ref_count.increment();
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
    T: ?Sized + 'static,
{
    fn drop(&mut self) {
        if let Some(obj) = self.obj.upgrade() {
            obj.ref_count.decrement();
        }
    }
}

pub struct GcRefGuard<'a, T>
where
    T: ?Sized + 'static,
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
        (*self.obj).as_ref()
    }
}

pub struct PinnedGcRef<T>
where
    T: ?Sized,
{
    obj: Rc<InnerType<T>>,
}

impl<T> PinnedGcRef<T>
where
    T: ?Sized + 'static,
{
    /// Private method to convert a `GcRef` into a `PinnedGcRef`.
    fn from_rc(obj: Rc<InnerType<T>>) -> Self {
        obj.pin_count.increment();
        Self { obj }
    }

    pub fn ref_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.obj, &other.obj)
    }

    pub fn to_ref(&self) -> GcRef<T> {
        GcRef::from_rc(self.obj.clone())
    }

    pub fn into_ref(self, _env_lock: &CollectGuard) -> GcRef<T> {
        self.to_ref()
    }
}

impl<T> std::ops::Deref for PinnedGcRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.obj.contents
    }
}

impl<T> Clone for PinnedGcRef<T>
where
    T: GcTraceable + 'static,
{
    fn clone(&self) -> Self {
        PinnedGcRef::from_rc(self.obj.clone())
    }
}

impl<T> Drop for PinnedGcRef<T>
where
    T: ?Sized,
{
    fn drop(&mut self) {
        self.obj.pin_count.decrement();
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
