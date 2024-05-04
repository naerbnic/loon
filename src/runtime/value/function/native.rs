#![allow(dead_code)]

use std::rc::Rc;

use crate::{
    refs::GcRef,
    runtime::{
        context::GlobalEnv, error::Result, stack_frame::LocalStack, value::Value, EvalContext,
    },
};

use super::Function;

pub struct TailCall {
    function: GcRef<Function>,
    num_args: u32,
}

pub struct CallWithContinuation {
    function: GcRef<Function>,
    num_args: u32,
    continuation: NativeFunctionPtr,
}

pub enum NativeFunctionResult {
    ReturnValue(u32),
    TailCall(TailCall),
    CallWithContinuation(CallWithContinuation),
}

pub struct NativeFunctionContext<'a> {
    global_context: &'a GlobalEnv,
    local_stack: &'a mut LocalStack,
}

impl<'a> NativeFunctionContext<'a> {
    pub fn call(&mut self, num_args: u32) -> Result<u32> {
        let function = self.local_stack.pop()?.as_function()?.clone();
        let mut eval_context = EvalContext::new(self.global_context, self.local_stack);
        eval_context.run(function, num_args)
    }

    pub fn return_with(self, num_args: u32) -> NativeFunctionResult {
        NativeFunctionResult::ReturnValue(num_args)
    }

    pub fn tail_call(self, num_args: u32) -> Result<NativeFunctionResult> {
        let function = self.local_stack.pop()?.as_function()?.clone();
        Ok(NativeFunctionResult::TailCall(TailCall {
            function,
            num_args,
        }))
    }

    pub fn call_with_continuation(
        self,
        num_args: u32,
        continuation: NativeFunctionPtr,
    ) -> Result<NativeFunctionResult> {
        let function = self.local_stack.pop()?.as_function()?.clone();
        Ok(NativeFunctionResult::CallWithContinuation(
            CallWithContinuation {
                function,
                num_args,
                continuation,
            },
        ))
    }
}

pub trait NativeFunction {
    fn call(&self, ctxt: NativeFunctionContext) -> Result<NativeFunctionResult>;
}

#[derive(Clone)]
pub struct NativeFunctionPtr(Rc<dyn NativeFunction>);
