use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use crate::gc::GcTraceable;

use super::{
    error::Result,
    global_env::GlobalEnv,
    instructions::FrameChange,
    stack_frame::{LocalStack, StackFrame},
    value::Function,
};

struct Inner {
    call_stack: RefCell<Vec<StackFrame>>,
}

impl GcTraceable for Inner {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: crate::gc::GcRefVisitor,
    {
        for frame in self.call_stack.borrow().iter() {
            frame.trace(visitor);
        }
    }
}

pub struct EvalContext<'a> {
    global_context: &'a GlobalEnv,
    parent_stack: &'a LocalStack,
    inner: Rc<Inner>,
}

impl<'a> EvalContext<'a> {
    pub fn new(global_context: &'a GlobalEnv, parent_stack: &'a LocalStack) -> Self {
        let inner = Rc::new(Inner {
            call_stack: RefCell::new(Vec::new()),
        });
        global_context.add_eval_context_contents(EvalContextContents(Rc::downgrade(&inner)));
        EvalContext {
            global_context,
            parent_stack,
            inner,
        }
    }

    pub fn run(&mut self, function: Function, num_args: u32) -> Result<u32> {
        let stack_frame = function.make_stack_frame(self.parent_stack.drain_top_n(num_args)?)?;
        self.inner.call_stack.borrow_mut().push(stack_frame);
        loop {
            let frame = self.inner.call_stack.borrow().last().unwrap().clone();
            match frame.run_to_frame_change(self.global_context)? {
                FrameChange::Return(num_returns) => {
                    let prev_frame = self
                        .inner
                        .call_stack
                        .borrow_mut()
                        .pop()
                        .expect("Call stack is empty.");
                    match self.inner.call_stack.borrow().last() {
                        Some(frame) => {
                            frame.push_sequence(prev_frame.drain_top_n(num_returns)?);
                        }
                        None => {
                            self.parent_stack
                                .push_sequence(prev_frame.drain_top_n(num_returns)?);
                            return Ok(num_returns);
                        }
                    }
                }
                FrameChange::Call(call) => {
                    let function = call.function;
                    let args = frame.drain_top_n(call.num_args)?;
                    let stack_frame = function.make_stack_frame(args)?;
                    self.inner.call_stack.borrow_mut().push(stack_frame);
                }
            }
        }
    }
}

impl Drop for EvalContext<'_> {
    fn drop(&mut self) {
        let contents = EvalContextContents(Rc::downgrade(&self.inner));
        self.global_context.remove_eval_context_contents(contents);
    }
}

pub struct EvalContextContents(Weak<Inner>);

impl EvalContextContents {
    pub fn get_ptr(&self) -> *const () {
        self.0.as_ptr() as *const ()
    }
}

impl GcTraceable for EvalContextContents {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: crate::gc::GcRefVisitor,
    {
        if let Some(ptr) = self.0.upgrade() {
            ptr.trace(visitor);
        }
    }
}
