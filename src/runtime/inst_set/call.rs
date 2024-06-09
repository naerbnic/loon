use crate::{
    binary::instructions::CallInstruction,
    runtime::{
        context::InstEvalContext,
        error::RuntimeError,
        instructions::{FunctionCallResult, InstEval, InstructionResult, InstructionTarget},
        stack_frame::LocalStack,
    },
};

#[derive(Clone, Debug)]
pub struct Call(CallInstruction);

impl Call {
    pub fn new(call_inst: CallInstruction) -> Self {
        Self(call_inst)
    }
}

impl InstEval for Call {
    fn execute(
        &self,
        _ctxt: &InstEvalContext,
        _stack: &LocalStack,
    ) -> std::prelude::v1::Result<InstructionResult, RuntimeError> {
        Ok(InstructionResult::Call(FunctionCallResult::new(
            self.0.num_args,
            InstructionTarget::Step,
        )))
    }
}
