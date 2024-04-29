use super::{
    error::Result,
    instructions::{InstEval, InstructionResult},
};

pub struct Return;

impl InstEval for Return {
    fn execute(&self, _stack: &mut super::stack_frame::LocalStack) -> Result<InstructionResult> {
        Ok(InstructionResult::Return)
    }
}
