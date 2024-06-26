use crate::runtime::{
    context::InstEvalContext,
    error::{Result, RuntimeError},
    instructions::{InstEval, InstructionResult},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct ReturnDynamic;

impl InstEval for ReturnDynamic {
    fn execute(&self, _ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let num_args = stack.pop()?.as_compact_integer()?;
        Ok(InstructionResult::Return(u32::try_from(num_args).map_err(
            |e| RuntimeError::new_operation_precondition_error(format!("Conversion failure: {e}")),
        )?))
    }
}
