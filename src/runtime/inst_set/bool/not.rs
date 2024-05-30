use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
    value::PinnedValue,
};

#[derive(Clone, Debug)]
pub struct BoolNot;

impl InstEval for BoolNot {
    fn execute(&self, _ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let b1 = stack.pop()?.as_bool()?;
        stack.push(PinnedValue::new_bool(!b1));
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
