use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct ListGet;

impl InstEval for ListGet {
    fn execute(&self, ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let lock = ctxt.get_env().lock_collect();
        let list_value = stack.pop(&lock)?;
        let list = list_value.as_list()?;
        let index = stack.pop(&lock)?.as_compact_integer()?;
        let list = list.borrow();
        let elem = list.at(index as usize);
        stack.push(elem);
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
