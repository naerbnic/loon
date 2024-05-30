use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct Add;

impl InstEval for Add {
    fn execute(&self, _ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let a = stack.pop()?;
        let b = stack.pop()?;
        // Right now, only implement integer addition.
        let result = a.add_owned(b)?;
        stack.push(result);
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
