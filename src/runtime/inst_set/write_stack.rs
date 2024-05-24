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
    fn execute(&self, ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let env_lock = ctxt.get_env().lock_collect();
        let top_value = stack.pop(&env_lock)?;
        stack.set_at_index(self.0, top_value)?;
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
