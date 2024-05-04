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
    fn execute(&self, ctxt: &InstEvalContext, stack: &mut LocalStack) -> Result<InstructionResult> {
        let value = stack.pop()?;
        ctxt.set_global(self.0, value)?;
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
