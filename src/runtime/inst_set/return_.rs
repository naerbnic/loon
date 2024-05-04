use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct Return(u32);

impl Return {
    pub fn new(num_returns: u32) -> Self {
        Return(num_returns)
    }
}

impl InstEval for Return {
    fn execute(
        &self,
        _ctxt: &InstEvalContext,
        _stack: &mut LocalStack,
    ) -> Result<InstructionResult> {
        Ok(InstructionResult::Return(self.0))
    }
}
