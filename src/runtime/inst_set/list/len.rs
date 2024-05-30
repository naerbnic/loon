use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
    value::PinnedValue,
};

#[derive(Clone, Debug)]
pub struct ListLen;

impl InstEval for ListLen {
    fn execute(&self, _ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let list_value = stack.pop()?;
        let list = list_value.as_list()?;
        let len = list.len();
        stack.push(PinnedValue::new_integer(i64::try_from(len).unwrap().into()));
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
