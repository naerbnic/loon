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
        _ctxt: &InstEvalContext,
        _stack: &LocalStack,
    ) -> std::prelude::v1::Result<InstructionResult, RuntimeError> {
        Ok(InstructionResult::TailCall(FunctionCallResult::new(
            self.0,
            InstructionTarget::Step,
        )))
    }
}
