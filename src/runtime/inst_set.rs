use super::{
    error::RuntimeError,
    instructions::{InstEval, InstructionResult},
};

pub struct Return;

impl InstEval for Return {
    fn execute(
        &self,
        stack: &mut super::stack_frame::LocalStack,
    ) -> Result<InstructionResult, RuntimeError> {
        Ok(InstructionResult::Return)
    }
}
