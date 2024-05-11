#![allow(dead_code)]

use std::rc::Rc;

use crate::{
    gc::{GcTraceable, PinnedGcRef},
    runtime::{
        error::Result,
        eval_context::EvalContext,
        global_env::{GlobalEnv, GlobalEnvLock},
        stack_frame::{LocalStack, StackContext, StackFrame},
        value::Value,
    },
    util::sequence::Sequence,
};

use super::Function;

pub(crate) struct TailCall {
    function: PinnedGcRef<Function>,
    num_args: u32,
}

pub(crate) struct CallWithContinuation {
    function: PinnedGcRef<Function>,
    num_args: u32,
    continuation: NativeFunctionPtr,
}

impl CallWithContinuation {
    pub fn function(&self) -> &PinnedGcRef<Function> {
        &self.function
    }

    pub fn num_args(&self) -> u32 {
        self.num_args
    }

    pub fn continuation(&self) -> &NativeFunctionPtr {
        &self.continuation
    }
}

pub struct NativeFunctionResult(pub(crate) NativeFunctionResultInner);

pub enum NativeFunctionResultInner {
    ReturnValue(u32),
    TailCall(TailCall),
    CallWithContinuation(CallWithContinuation),
}

pub struct NativeFunctionContext<'a> {
    global_context: &'a GlobalEnv,
    local_stack: &'a LocalStack,
}

impl<'a> NativeFunctionContext<'a> {
    pub(crate) fn new(global_context: &'a GlobalEnv, local_stack: &'a LocalStack) -> Self {
        NativeFunctionContext {
            global_context,
            local_stack,
        }
    }

    pub fn stack(&mut self) -> StackContext {
        StackContext::new(&self.global_context.lock_collect(), self.local_stack)
    }

    pub fn call(&mut self, num_args: u32) -> Result<u32> {
        let function = {
            let lock = self.global_context.lock_collect();
            self.local_stack.pop(&lock)?.as_function()?.pin()
        };
        let mut eval_context = EvalContext::new(self.global_context, self.local_stack);
        eval_context.run(function, num_args)
    }

    pub fn return_with(self, num_args: u32) -> NativeFunctionResult {
        NativeFunctionResult(NativeFunctionResultInner::ReturnValue(num_args))
    }

    pub fn tail_call(self, num_args: u32) -> Result<NativeFunctionResult> {
        let function = {
            let lock = self.global_context.lock_collect();
            self.local_stack.pop(&lock)?.as_function()?.pin()
        };
        Ok(NativeFunctionResult(NativeFunctionResultInner::TailCall(
            TailCall { function, num_args },
        )))
    }

    pub fn call_with_continuation(
        self,
        num_args: u32,
        continuation: NativeFunctionPtr,
    ) -> Result<NativeFunctionResult> {
        let function = {
            let lock = self.global_context.lock_collect();
            self.local_stack.pop(&lock)?.as_function()?.pin()
        };
        Ok(NativeFunctionResult(
            NativeFunctionResultInner::CallWithContinuation(CallWithContinuation {
                function,
                num_args,
                continuation,
            }),
        ))
    }
}

pub trait NativeFunction {
    fn call(&self, ctxt: NativeFunctionContext) -> Result<NativeFunctionResult>;
}

impl<F> NativeFunction for F
where
    F: Fn(NativeFunctionContext) -> Result<NativeFunctionResult>,
{
    fn call(&self, ctxt: NativeFunctionContext) -> Result<NativeFunctionResult> {
        self(ctxt)
    }
}

#[derive(Clone)]
pub struct NativeFunctionPtr(Rc<dyn NativeFunction>);

impl NativeFunctionPtr {
    pub fn new<T>(func: T) -> Self
    where
        T: NativeFunction + 'static,
    {
        NativeFunctionPtr(Rc::new(func))
    }

    pub fn call(&self, ctxt: NativeFunctionContext) -> Result<NativeFunctionResult> {
        self.0.call(ctxt)
    }

    pub(crate) fn make_stack_frame(
        &self,
        env_lock: &GlobalEnvLock,
        args: impl Sequence<Value>,
        local_stack: LocalStack,
    ) -> Result<StackFrame> {
        local_stack.push_sequence(args);
        Ok(StackFrame::new_native(env_lock, self.clone(), local_stack))
    }
}

impl GcTraceable for NativeFunctionPtr {
    fn trace<V>(&self, _visitor: &mut V)
    where
        V: crate::gc::GcRefVisitor,
    {
        // Nothing to trace here
    }
}
