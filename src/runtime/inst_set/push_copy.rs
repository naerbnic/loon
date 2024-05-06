use crate::{
    binary::instructions::StackIndex,
    runtime::{
        context::InstEvalContext,
        error::Result,
        instructions::{InstEval, InstructionResult, InstructionTarget},
        stack_frame::LocalStack,
    },
};

#[derive(Clone, Debug)]
pub struct PushCopy(StackIndex);

impl PushCopy {
    pub fn new(index: StackIndex) -> Self {
        PushCopy(index)
    }
}

impl InstEval for PushCopy {
    fn execute(&self, _ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let value = stack.get_at_index(self.0)?;
        stack.push(value.clone());
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
