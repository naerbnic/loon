use std::cell::RefCell;

use crate::gc::{GcRef, GcTraceable, PinnedGcRef};

use super::{
    error::Result,
    global_env::GlobalEnv,
    instructions::FrameChange,
    stack_frame::{LocalStack, StackFrame},
    value::Function,
    RuntimeError,
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
            let stack_frame = self.global_context.with_value_buffer(|buffer| {
                self.parent_stack.drain_top_n(num_args, buffer)?;
                function.make_stack_frame(self.global_context, buffer)
            })?;
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
                        self.global_context.with_value_buffer(|buf| {
                            prev_frame.drain_top_n(num_returns, buf)?;
                            frame.borrow().push_iter(self.global_context, buf.drain(..));
                            Ok::<_, RuntimeError>(())
                        })?;
                    } else {
                        return self.global_context.with_value_buffer(|buf| {
                            prev_frame.drain_top_n(num_returns, buf)?;
                            self.parent_stack
                                .push_iter(self.global_context, buf.drain(..));
                            Ok(num_returns)
                        });
                    }
                }
                FrameChange::Call(call) => {
                    let stack_frame = self.global_context.with_value_buffer(|buf| {
                        frame.drain_top_n(call.num_args, buf)?;
                        let function = frame.pop()?.as_function()?.clone();
                        let stack_frame = function.make_stack_frame(self.global_context, buf)?;
                        Ok::<_, RuntimeError>(stack_frame)
                    })?;
                    self.global_context.with_lock(|lock| {
                        self.inner
                            .call_stack
                            .borrow_mut()
                            .push(stack_frame.into_ref(lock.guard()))
                    });
                }
                FrameChange::TailCall(call) => {
                    let stack_frame = self.global_context.with_value_buffer(|buf| {
                        frame.drain_top_n(call.num_args, buf)?;
                        let function = frame.pop()?.as_function()?.clone();
                        let stack_frame = function.make_stack_frame(self.global_context, buf)?;
                        Ok::<_, RuntimeError>(stack_frame)
                    })?;
                    let mut call_stack = self.inner.call_stack.borrow_mut();
                    call_stack.pop();
                    self.global_context.with_lock(|lock| {
                        call_stack.push(stack_frame.into_ref(lock.guard()));
                    });
                }
                FrameChange::YieldCall(_call) => todo!(),
            }
        }
    }
}
