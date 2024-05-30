use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
    value::{List, PinnedValue},
};

#[derive(Clone, Debug)]
pub struct ListNew;

impl InstEval for ListNew {
    fn execute(&self, ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let list = PinnedValue::new_list(List::new(ctxt.get_env()));
        stack.push(list);
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
