use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct ListAppend;

impl InstEval for ListAppend {
    fn execute(
        &self,
        _ctxt: &InstEvalContext,
        stack: &mut LocalStack,
    ) -> Result<InstructionResult> {
        let list_value = stack.pop()?;
        let list = list_value.as_list()?;
        let value = stack.pop()?;
        list.with(|list| list.append(value));
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
