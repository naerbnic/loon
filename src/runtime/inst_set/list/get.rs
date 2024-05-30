use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct ListGet;

impl InstEval for ListGet {
    fn execute(&self, _ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let list_value = stack.pop()?;
        let list = list_value.as_list()?;
        let index = stack.pop()?.as_compact_integer()?;
        let elem = list.at(index as usize);
        stack.push(elem);
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
