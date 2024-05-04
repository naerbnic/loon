use crate::runtime::{
    context::InstEvalContext,
    error::RuntimeError,
    instructions::{FunctionCallResult, InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct CallDynamic;

impl InstEval for CallDynamic {
    fn execute(
        &self,
        _ctxt: &InstEvalContext,
        stack: &mut LocalStack,
    ) -> std::prelude::v1::Result<InstructionResult, RuntimeError> {
        let func = stack.pop()?.as_function()?.clone();
        let num_args = stack.pop()?.as_compact_integer()?;
        let num_args = u32::try_from(num_args).map_err(|_| {
            if num_args < 0 {
                RuntimeError::new_operation_precondition_error("Number of arguments is negative.")
            } else {
                RuntimeError::new_operation_precondition_error("Number of arguments is too large.")
            }
        })?;
        let mut args = Vec::with_capacity(usize::try_from(num_args).unwrap());
        for _ in 0..num_args {
            let arg = stack.pop()?;
            args.push(arg);
        }
        Ok(InstructionResult::Call(FunctionCallResult::new(
            func.clone(),
            args,
            InstructionTarget::Step,
        )))
    }
}
