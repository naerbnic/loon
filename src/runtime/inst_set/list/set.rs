use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct ListSet;

impl InstEval for ListSet {
    fn execute(&self, ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let lock = ctxt.get_env().lock_collect();
        let list_value = stack.pop(&lock)?;
        let list = list_value.as_list()?.borrow();
        let index = stack.pop(&lock)?.as_compact_integer()?;
        let elem = stack.pop(&lock)?;
        list.set(index as u32, elem)?;
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
