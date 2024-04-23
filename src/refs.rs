use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    rc::{Rc, Weak},
};

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
struct PtrKey(*const ());

impl PtrKey {
    pub fn from_rc<T>(p: &Rc<RefCell<T>>) -> Self
    where
        T: ?Sized,
    {
        PtrKey(Rc::as_ptr(p) as *const ())
    }

    pub fn from_weak<T>(p: &Weak<RefCell<T>>) -> Option<Self>
    where
        T: ?Sized,
    {
        Some(PtrKey::from_rc(&p.upgrade()?))
    }
}

trait ObjectInfo {
    fn trace(&self, ptr_visitor: &mut dyn FnMut(PtrKey));
    fn destroy(self: Box<Self>);
}

struct PtrVisitor<'a>(&'a mut dyn FnMut(PtrKey));

impl GcRefVisitor for PtrVisitor<'_> {
    fn visit<T>(&mut self, obj: &Ref<T>)
    where
        T: ?Sized,
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

pub struct RefContext {
    inner: Rc<RefCell<ContextInner>>,
}

impl RefContext {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(ContextInner {
                live_objects: HashMap::new(),
            })),
        }
    }

    pub fn create_ref<T>(&self, value: T) -> Ref<T>
    where
        T: GcTraceable + 'static,
    {
        let owned_obj = Rc::new(RefCell::new(value));
        // We use the pointer as a key to the object in the HashMap.
        let ptr_id = PtrKey::from_rc(&owned_obj);
        let obj = Rc::downgrade(&owned_obj);

        let obj_info = ObjectInfoImpl::new(owned_obj);
        {
            let mut inner = self.inner.borrow_mut();
            inner.live_objects.insert(ptr_id, Box::new(obj_info));
        }

        Ref { obj }
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

impl Default for RefContext {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WeakRefContext {
    inner: Weak<RefCell<ContextInner>>,
}

impl WeakRefContext {
    pub fn upgrade(&self) -> Option<RefContext> {
        self.inner.upgrade().map(|inner| RefContext { inner })
    }
}

pub struct Ref<T>
where
    T: ?Sized,
{
    obj: Weak<RefCell<T>>,
}

impl<T> Ref<T>
where
    T: ?Sized,
{
    pub fn try_with_mut<F, R>(&self, body: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        let obj = self.obj.upgrade()?;
        let mut obj = obj.borrow_mut();
        Some(body(&mut *obj))
    }

    pub fn with_mut<F, R>(&self, body: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        self.try_with_mut(body)
            .expect("object was already destroyed")
    }
}

impl<T> Clone for Ref<T>
where
    T: ?Sized,
{
    fn clone(&self) -> Self {
        Self {
            obj: self.obj.clone(),
        }
    }
}

pub struct GcRoots {
    roots: HashSet<PtrKey>,
}

impl GcRoots {
    pub fn new() -> Self {
        Self {
            roots: HashSet::new(),
        }
    }

    pub fn add<T>(&mut self, obj: &Ref<T>)
    where
        T: ?Sized,
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

pub trait GcRefVisitor {
    fn visit<T>(&mut self, obj: &Ref<T>)
    where
        T: ?Sized;
}

pub trait GcTraceable {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor;
}
