use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct ReturnDynamic;

impl InstEval for ReturnDynamic {
    fn execute(
        &self,
        _ctxt: &InstEvalContext,
        _stack: &mut LocalStack,
    ) -> Result<InstructionResult> {
        Ok(InstructionResult::Return)
    }
}
