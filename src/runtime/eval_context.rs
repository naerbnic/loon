use std::cell::RefCell;

use crate::gc::{GcRef, GcTraceable, PinnedGcRef};

use super::{
    error::Result,
    global_env::GlobalEnv,
    instructions::FrameChange,
    stack_frame::{LocalStack, PinnedValueList, StackFrame},
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
    temp_stack: &'a mut PinnedValueList,
}

impl<'a> EvalContext<'a> {
    pub fn new(
        global_context: &'a GlobalEnv,
        parent_stack: &'a PinnedGcRef<LocalStack>,
        temp_stack: &'a mut PinnedValueList,
    ) -> Self {
        let inner = global_context.create_pinned_ref(Inner {
            call_stack: RefCell::new(Vec::new()),
        });

        EvalContext {
            global_context,
            parent_stack,
            inner,
            temp_stack,
        }
    }

    pub fn run(&mut self, function: &PinnedGcRef<Function>, num_args: u32) -> Result<u32> {
        {
            self.temp_stack.clear();
            self.parent_stack.drain_top_n(num_args, self.temp_stack)?;
            let stack_frame = function.make_stack_frame(self.global_context, self.temp_stack)?;
            self.global_context.with_lock(|lock| {
                self.inner
                    .call_stack
                    .borrow_mut()
                    .push(stack_frame.into_ref(lock.guard()))
            });
        }
        loop {
            let frame = self.inner.call_stack.borrow().last().unwrap().pin();
            match frame.run_to_frame_change(self.global_context, self.temp_stack)? {
                FrameChange::Return(num_returns) => {
                    let prev_frame = self
                        .inner
                        .call_stack
                        .borrow_mut()
                        .pop()
                        .expect("Call stack is empty.")
                        .pin();
                    if let Some(frame) = self.inner.call_stack.borrow().last() {
                        self.temp_stack.clear();
                        prev_frame.drain_top_n(num_returns, self.temp_stack)?;
                        frame
                            .borrow()
                            .push_iter(self.global_context, self.temp_stack.drain(..));
                    } else {
                        self.temp_stack.clear();
                        prev_frame.drain_top_n(num_returns, self.temp_stack)?;
                        self.parent_stack
                            .push_iter(self.global_context, self.temp_stack.drain(..));
                        return Ok(num_returns);
                    }
                }
                FrameChange::Call(call) => {
                    self.temp_stack.clear();
                    frame.drain_top_n(call.num_args, self.temp_stack)?;
                    let function = frame.pop()?.as_function()?.clone();
                    let stack_frame =
                        function.make_stack_frame(self.global_context, self.temp_stack)?;
                    self.global_context.with_lock(|lock| {
                        self.inner
                            .call_stack
                            .borrow_mut()
                            .push(stack_frame.into_ref(lock.guard()))
                    });
                }
                FrameChange::TailCall(call) => {
                    self.temp_stack.clear();
                    frame.drain_top_n(call.num_args, self.temp_stack)?;
                    let function = frame.pop()?.as_function()?.clone();
                    let stack_frame =
                        function.make_stack_frame(self.global_context, self.temp_stack)?;
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
