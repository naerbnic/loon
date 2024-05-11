use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct SetGlobal(u32);

impl SetGlobal {
    pub fn new(index: u32) -> Self {
        SetGlobal(index)
    }
}

impl InstEval for SetGlobal {
    fn execute(&self, ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let lock = ctxt.get_env().lock_collect();
        let value = stack.pop(&lock)?;
        ctxt.set_global(self.0, value)?;
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
