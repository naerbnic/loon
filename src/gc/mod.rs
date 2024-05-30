//! This module defines a simple garbage collector that uses a basic mark-and-sweep
//! algorithm.
//!
//! As a prototype, it is more important for the interface to be ergonomic,
//! rather than performant.

mod core;
mod counter;

pub use core::{CollectGuard, GcEnv, GcRef, GcRefVisitor, GcTraceable, PinnedGcRef};

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

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
        let env = GcEnv::new(100);
        let i_ref = env.create_pinned_ref(4).to_ref();
        let val = *i_ref.borrow();
        assert_eq!(val, 4);
    }

    #[test]
    fn test_simple_gc() {
        let env = GcEnv::new(100);
        let i_ref = env.create_pinned_ref(4);
        let i_ref = i_ref.to_ref();
        env.force_collect();
        let val = *i_ref.borrow();
        assert_eq!(val, 4);
    }

    #[test]
    fn test_simple_gc_collect() {
        let env = GcEnv::new(100);
        let i_ref = env.create_pinned_ref(4).to_ref();
        env.force_collect();
        let val = i_ref.try_borrow();
        assert!(val.is_none());
    }

    #[test]
    fn loop_collects() {
        let env = GcEnv::new(100);

        let (node1, drop1) = Node::new();
        let (node2, drop2) = Node::new();
        let node2_ref = env.create_pinned_ref(node2);
        let node1_ref = env.create_pinned_ref(node1);
        node1_ref.add_child(node2_ref.to_ref());
        node2_ref.add_child(node1_ref.to_ref());
        assert!(!drop1());
        assert!(!drop2());

        drop(node2_ref);

        // With either of the two, both should not be collected.
        env.force_collect();
        assert!(!drop1());
        assert!(!drop2());

        drop(node1_ref);
        env.force_collect();
        assert!(drop1());
        assert!(drop2());
    }
}
