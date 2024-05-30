use std::cell::RefCell;

use crate::gc::{GcRef, GcTraceable, PinnedGcRef};

use super::{
    error::Result,
    global_env::GlobalEnv,
    instructions::FrameChange,
    stack_frame::{LocalStack, StackFrame},
    value::Function,
};

struct Inner {
    call_stack: RefCell<Vec<GcRef<StackFrame>>>,
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
    parent_stack: &'a PinnedGcRef<LocalStack>,
    inner: PinnedGcRef<Inner>,
}

impl<'a> EvalContext<'a> {
    pub fn new(global_context: &'a GlobalEnv, parent_stack: &'a PinnedGcRef<LocalStack>) -> Self {
        let inner = global_context.create_pinned_ref(Inner {
            call_stack: RefCell::new(Vec::new()),
        });
        EvalContext {
            global_context,
            parent_stack,
            inner,
        }
    }

    pub fn run(&mut self, function: &PinnedGcRef<Function>, num_args: u32) -> Result<u32> {
        {
            let stack_frame = function.make_stack_frame(
                self.global_context,
                self.parent_stack.drain_top_n(num_args)?,
            )?;
            self.global_context.with_lock(|lock| {
                self.inner
                    .call_stack
                    .borrow_mut()
                    .push(stack_frame.into_ref(lock.guard()))
            });
        }
        loop {
            let frame = self.inner.call_stack.borrow().last().unwrap().pin();
            match frame.run_to_frame_change(self.global_context)? {
                FrameChange::Return(num_returns) => {
                    let prev_frame = self
                        .inner
                        .call_stack
                        .borrow_mut()
                        .pop()
                        .expect("Call stack is empty.")
                        .pin();
                    if let Some(frame) = self.inner.call_stack.borrow().last() {
                        frame.borrow().push_sequence(
                            self.global_context,
                            prev_frame.drain_top_n(num_returns)?,
                        );
                    } else {
                        self.parent_stack.push_sequence(
                            self.global_context,
                            prev_frame.drain_top_n(num_returns)?,
                        );
                        return Ok(num_returns);
                    }
                }
                FrameChange::Call(call) => {
                    let function = call.function;
                    let args = frame.drain_top_n(call.num_args)?;
                    let stack_frame = function.make_stack_frame(self.global_context, args)?;
                    self.global_context.with_lock(|lock| {
                        self.inner
                            .call_stack
                            .borrow_mut()
                            .push(stack_frame.into_ref(lock.guard()))
                    });
                }
                FrameChange::TailCall(call) => {
                    let function = call.function;
                    let args = frame.drain_top_n(call.num_args)?;
                    let stack_frame = function.make_stack_frame(self.global_context, args)?;
                    let mut call_stack = self.inner.call_stack.borrow_mut();
                    call_stack.pop();
                    self.global_context.with_lock(|lock| {
                        call_stack.push(stack_frame.into_ref(lock.guard()));
                    });
                }
            }
        }
    }
}
