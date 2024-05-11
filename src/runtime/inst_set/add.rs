use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct Add;

impl InstEval for Add {
    fn execute(&self, ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let lock = ctxt.get_env().lock_collect();
        let a = stack.pop(&lock)?;
        let b = stack.pop(&lock)?;
        // Right now, only implement integer addition.
        let result = a.add_owned(b)?;
        stack.push(result);
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
