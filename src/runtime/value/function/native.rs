#![allow(dead_code)]

use std::rc::Rc;

use crate::{
    gc::{GcTraceable, PinnedGcRef},
    runtime::{
        error::Result,
        eval_context::EvalContext,
        global_env::GlobalEnv,
        stack_frame::{LocalStack, StackContext, StackFrame},
        value::PinnedValue,
    },
    util::sequence::Sequence,
};

use super::Function;

pub(crate) struct TailCall {
    pub function: PinnedGcRef<Function>,
    pub num_args: u32,
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

pub(crate) struct YieldCall {
    /// The function that will be called with the continuation as an argument.
    pub function: PinnedGcRef<Function>,
}

pub struct NativeFunctionResult(pub(crate) NativeFunctionResultInner);

pub enum NativeFunctionResultInner {
    /// Returns a set of values to the caller. The values are found on the
    /// top of the stack.
    ReturnValue(u32),

    /// Tail call to another function. This is equivalent to calling the
    /// referenced function, and then returning the result.
    TailCall(TailCall),

    /// Call with a continuation. This must provide another function to call
    /// when the provided function is done. The continuation function will
    /// receive the return values of the provided function as arguments.
    CallWithContinuation(CallWithContinuation),

    /// Yield to the closest enclosing continuation scope, or the top-level
    /// if that does not exist.
    YieldCall(YieldCall),
}

pub struct NativeFunctionContext<'a> {
    global_context: &'a GlobalEnv,
    local_stack: &'a PinnedGcRef<LocalStack>,
}

impl<'a> NativeFunctionContext<'a> {
    pub(crate) fn new(
        global_context: &'a GlobalEnv,
        local_stack: &'a PinnedGcRef<LocalStack>,
    ) -> Self {
        NativeFunctionContext {
            global_context,
            local_stack,
        }
    }

    pub fn stack(&mut self) -> StackContext {
        StackContext::new(self.global_context, self.local_stack.clone())
    }

    pub fn call(&mut self, num_args: u32) -> Result<u32> {
        let function = self.local_stack.pop()?.as_function()?.clone();
        let mut eval_context = EvalContext::new(self.global_context, self.local_stack);
        eval_context.run(&function, num_args)
    }

    pub fn return_with(self, num_args: u32) -> NativeFunctionResult {
        NativeFunctionResult(NativeFunctionResultInner::ReturnValue(num_args))
    }

    pub fn tail_call(self, num_args: u32) -> Result<NativeFunctionResult> {
        let function = self.local_stack.pop()?.as_function()?.clone();
        Ok(NativeFunctionResult(NativeFunctionResultInner::TailCall(
            TailCall { function, num_args },
        )))
    }

    pub fn call_with_continuation(
        self,
        num_args: u32,
        continuation: NativeFunctionPtr,
    ) -> Result<NativeFunctionResult> {
        let function = self.local_stack.pop()?.as_function()?.clone();
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
        env: &GlobalEnv,
        args: impl Sequence<PinnedValue>,
        local_stack: PinnedGcRef<LocalStack>,
    ) -> Result<PinnedGcRef<StackFrame>> {
        local_stack.push_sequence(env, args);
        Ok(StackFrame::new_native(env, self.clone(), local_stack))
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
