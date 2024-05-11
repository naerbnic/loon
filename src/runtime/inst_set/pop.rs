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
    fn execute(&self, _ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        stack.pop_n(self.0 as usize)?;
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
