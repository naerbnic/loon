use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct Pop(u32);

impl Pop {
    pub fn new(count: u32) -> Self {
        Pop(count)
    }
}

impl InstEval for Pop {
    fn execute(
        &self,
        _ctxt: &InstEvalContext,
        stack: &mut LocalStack,
    ) -> Result<InstructionResult> {
        for _ in 0..self.0 {
            stack.pop()?;
        }
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
