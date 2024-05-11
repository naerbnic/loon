use crate::runtime::{
    context::InstEvalContext,
    error::RuntimeError,
    instructions::{FunctionCallResult, InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct TailCall(u32);

impl TailCall {
    pub fn new(num_args: u32) -> Self {
        Self(num_args)
    }
}

impl InstEval for TailCall {
    fn execute(
        &self,
        ctxt: &InstEvalContext,
        stack: &LocalStack,
    ) -> std::prelude::v1::Result<InstructionResult, RuntimeError> {
        let lock = ctxt.get_env().lock_collect();
        let func = stack.pop(&lock)?.as_function()?.clone();
        Ok(InstructionResult::TailCall(FunctionCallResult::new(
            func.pin(),
            self.0,
            InstructionTarget::Step,
        )))
    }
}
