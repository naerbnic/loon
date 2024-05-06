use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct PushGlobal(u32);

impl PushGlobal {
    pub fn new(index: u32) -> Self {
        PushGlobal(index)
    }
}

impl InstEval for PushGlobal {
    fn execute(&self, ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let value = ctxt.get_global(self.0)?;
        stack.push(value);
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
