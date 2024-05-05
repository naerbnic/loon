use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct ListSet;

impl InstEval for ListSet {
    fn execute(
        &self,
        _ctxt: &InstEvalContext,
        stack: &mut LocalStack,
    ) -> Result<InstructionResult> {
        let list_value = stack.pop()?;
        let list = list_value.as_list()?;
        let index = stack.pop()?.as_compact_integer()?;
        let elem = stack.pop()?;
        list.with(|list| list.set(index as u32, elem))?;
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
