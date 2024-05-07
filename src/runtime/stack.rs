use std::cell::RefCell;

use crate::gc::{GcRef, GcTraceable};

use super::stack_frame::StackFrame;

struct Inner {
    stack: RefCell<Vec<StackFrame>>,
}

impl GcTraceable for Inner {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: crate::gc::GcRefVisitor,
    {
        for frame in self.stack.borrow().iter() {
            frame.trace(visitor);
        }
    }
}

pub struct Stack(GcRef<Inner>);

impl GcTraceable for Stack {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: crate::gc::GcRefVisitor,
    {
        self.0.trace(visitor);
    }
}
