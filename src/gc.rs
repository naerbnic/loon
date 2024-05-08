//! This module defines a simple garbage collector that uses a basic mark-and-sweep
//! algorithm.
//!
//! As a prototype, it is more important for the interface to be ergonomic,
//! rather than performant.

mod core;

pub use core::{GcEnv, GcRef, GcRefVisitor, GcTraceable};

#[cfg(test)]
mod tests {
    use super::core::GcRoots;
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

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
