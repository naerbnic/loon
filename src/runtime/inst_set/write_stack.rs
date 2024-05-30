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
pub struct WriteStack(StackIndex);

impl WriteStack {
    pub fn new(index: StackIndex) -> Self {
        WriteStack(index)
    }
}

impl InstEval for WriteStack {
    fn execute(&self, _ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let top_value = stack.pop()?;
        stack.set_at_index(self.0, top_value)?;
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
