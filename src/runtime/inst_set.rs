use super::{
    error::RuntimeError,
    instructions::{Instruction, InstructionResult},
};

pub struct Return;

impl Instruction for Return {
    fn execute(
        &self,
        stack: &mut super::stack_frame::LocalStack,
    ) -> Result<InstructionResult, RuntimeError> {
        Ok(InstructionResult::Return)
    }
}
