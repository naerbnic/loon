//! This is a partial reimplementation of the Rc/Weak types, that allow for
//! features that we depend on for safe cyclic references.

use std::{
    cell::{Cell, UnsafeCell},
    mem::MaybeUninit,
    ptr::NonNull,
};

struct RcBox<T> {
    /// The number of `Weak` references to this value. This has an additional
    /// 1 added to it if the `strong` count is non-zero.
    weak: Cell<usize>,
    strong: Cell<usize>,
    value: UnsafeCell<MaybeUninit<T>>,
}

impl<T> RcBox<T> {
    pub fn inc_strong(&self) {
        let curr_strong = self.strong.get();
        debug_assert!(curr_strong > 0);
        self.strong.set(
            curr_strong
                .checked_add(1)
                .expect("Overflow in strong count."),
        );
    }

    pub fn inc_weak(&self) {
        self.weak.set(
            self.weak
                .get()
                .checked_add(1)
                .expect("Overflow in weak count."),
        );
    }

    pub fn dec_strong(&self) -> bool {
        let next_strong = self
            .strong
            .get()
            .checked_sub(1)
            .expect("Underflow in strong count.");
        self.strong.set(next_strong);
        if next_strong == 0 {
            unsafe {
                // Drop the value in place
                self.value.get().as_mut().unwrap().assume_init_drop();
            }
            self.dec_weak()
        } else {
            false
        }
    }

    pub fn dec_weak(&self) -> bool {
        let new_weak = self
            .weak
            .get()
            .checked_sub(1)
            .expect("Underflow in weak count.");
        self.weak.set(new_weak);
        new_weak == 0
    }

    pub fn is_upgradable(&self) -> bool {
        self.strong.get() >= 1
    }

    pub fn resurrect(&self, value: T) -> Result<(), T> {
        if self.strong.get() != 0 {
            return Err(value);
        }

        self.weak.set(self.weak.get() + 1);
        self.strong.set(1);
        unsafe {
            self.value.get().write(MaybeUninit::new(value));
        }
        Ok(())
    }
}

/// A pointer to the internal RcBox for an Rc.
struct RcBoxPtr<T> {
    ptr: NonNull<RcBox<T>>,
}

impl<T> RcBoxPtr<T> {
    pub fn new_empty() -> Self {
        let ptr = Box::into_raw(Box::new(RcBox {
            weak: Cell::new(1),
            strong: Cell::new(0),
            value: UnsafeCell::new(MaybeUninit::uninit()),
        }));

        RcBoxPtr {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
        }
    }

    pub fn new(value: T) -> Self {
        let ptr = Box::into_raw(Box::new(RcBox {
            weak: Cell::new(1),
            strong: Cell::new(1),
            value: UnsafeCell::new(MaybeUninit::new(value)),
        }));

        RcBoxPtr {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
        }
    }

    pub fn inc_strong(&self) {
        unsafe { self.ptr.as_ref() }.inc_strong();
    }

    pub fn inc_weak(&self) {
        unsafe { self.ptr.as_ref() }.inc_weak();
    }

    pub fn dec_strong(&self) -> bool {
        unsafe { self.ptr.as_ref() }.dec_strong()
    }

    pub fn dec_weak(&self) -> bool {
        unsafe { self.ptr.as_ref() }.dec_weak()
    }

    pub fn is_upgradable(&self) -> bool {
        unsafe { self.ptr.as_ref() }.is_upgradable()
    }

    pub fn resurrect(&self, value: T) -> Result<(), T> {
        unsafe { self.ptr.as_ref() }.resurrect(value)
    }

    pub fn get_value_ref(&self) -> &T {
        debug_assert!(unsafe { self.ptr.as_ref() }.strong.get() > 0);
        unsafe { &*self.ptr.as_ref().value.get().as_ref().unwrap().as_ptr() }
    }

    /// Deletes this RcBoxPtr, deallocating the RcBox.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it deallocates the RcBox, when other
    /// copies of this RcBoxPtr may still be in use. It tracks the correct
    /// usage of the weak count for this purpose, but depends on those counts
    /// being correct.
    pub unsafe fn delete(&self) {
        debug_assert!(unsafe { self.ptr.as_ref() }.weak.get() == 0);
        drop(Box::from_raw(self.ptr.as_ptr()));
    }
}

impl<T> Clone for RcBoxPtr<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
        }
    }
}

pub(crate) struct Rc<T> {
    ptr: RcBoxPtr<T>,
}

impl<T> Rc<T> {
    fn from_rc_box_ptr(ptr: RcBoxPtr<T>) -> Self {
        ptr.inc_strong();
        Rc { ptr }
    }

    pub fn new(value: T) -> Self {
        Rc {
            ptr: RcBoxPtr::new(value),
        }
    }

    pub fn downgrade(this: &Self) -> Weak<T> {
        Weak::from_rc_box_ptr(this.ptr.clone())
    }

    pub fn as_ptr(&self) -> *const T {
        self.ptr.get_value_ref()
    }
}

impl<T> std::ops::Deref for Rc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.ptr.get_value_ref()
    }
}

impl<T> Clone for Rc<T> {
    fn clone(&self) -> Self {
        Rc::from_rc_box_ptr(self.ptr.clone())
    }
}

impl<T> Drop for Rc<T> {
    fn drop(&mut self) {
        if self.ptr.dec_strong() {
            unsafe {
                self.ptr.delete();
            }
        }
    }
}

pub(crate) struct Weak<T> {
    ptr: RcBoxPtr<T>,
}

impl<T> Weak<T> {
    fn from_rc_box_ptr(ptr: RcBoxPtr<T>) -> Self {
        ptr.inc_weak();
        Weak { ptr }
    }

    /// Creates a new Weak reference that does not point to any value. If cloned,
    /// the new Weak reference will share the same storage. If the pointer is
    /// resurrected, then both pointers will be upgradable again.
    pub fn new() -> Self {
        Weak {
            ptr: RcBoxPtr::new_empty(),
        }
    }
    pub fn upgrade(&self) -> Option<Rc<T>> {
        if !self.ptr.is_upgradable() {
            return None;
        }
        Some(Rc::from_rc_box_ptr(self.ptr.clone()))
    }

    pub fn resurrect(&self, value: T) -> Result<Rc<T>, T> {
        self.ptr.resurrect(value)?;
        Ok(Rc {
            ptr: self.ptr.clone(),
        })
    }
}

impl<T> Clone for Weak<T> {
    fn clone(&self) -> Self {
        Weak::from_rc_box_ptr(self.ptr.clone())
    }
}

impl<T> Drop for Weak<T> {
    fn drop(&mut self) {
        if self.ptr.dec_weak() {
            unsafe {
                self.ptr.delete();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct DropChecker<T>(T, std::rc::Rc<Cell<bool>>);

    impl<T> DropChecker<T> {
        pub fn new(value: T) -> (DropChecker<T>, impl Fn() -> bool) {
            let rc = std::rc::Rc::new(Cell::new(false));
            (DropChecker(value, rc.clone()), move || rc.get())
        }

        pub fn get_value(&self) -> &T {
            &self.0
        }
    }

    impl<T> Drop for DropChecker<T> {
        fn drop(&mut self) {
            self.1.set(true);
        }
    }

    #[test]
    fn test_single_rc_lifecycle() {
        let (dc, is_dropped) = DropChecker::new(3);
        let tc = Rc::new(dc);
        assert_eq!(*tc.get_value(), 3);
        drop(tc);
        assert!(is_dropped());
    }

    #[test]
    fn test_dual_rc_lifecycle() {
        let (dc, is_dropped) = DropChecker::new(3);
        let tc1 = Rc::new(dc);
        let tc2 = tc1.clone();
        assert_eq!(*tc1.get_value(), 3);
        assert_eq!(*tc2.get_value(), 3);
        drop(tc1);
        assert!(!is_dropped());
        drop(tc2);
        assert!(is_dropped());
    }

    #[test]
    fn test_weak_ptr_upgrade() {
        let (dc, is_dropped) = DropChecker::new(3);
        let tc = Rc::new(dc);
        let tw = Rc::downgrade(&tc);
        let tc2 = tw.upgrade().unwrap();
        assert_eq!(*tc2.get_value(), 3);
        drop(tc);
        assert!(!is_dropped());
    }

    #[test]
    fn test_weak_ptr_drop_original() {
        let (dc, is_dropped) = DropChecker::new(3);
        let tc = Rc::new(dc);
        let tw = Rc::downgrade(&tc);
        drop(tc);
        assert!(is_dropped());
        assert!(tw.upgrade().is_none());
    }

    #[test]
    fn test_weak_ptr_resurrect() {
        let w1 = Weak::new();
        let w2 = w1.clone();

        let (dc, is_dropped) = DropChecker::new(3);
        let tc = w1.resurrect(dc).unwrap();
        assert!(!is_dropped());
        assert_eq!(w1.upgrade().unwrap().get_value(), &3);
        assert_eq!(w2.upgrade().unwrap().get_value(), &3);
        drop(tc);
        assert!(is_dropped());
    }
}
