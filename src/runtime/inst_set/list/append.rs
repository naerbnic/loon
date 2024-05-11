use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct ListAppend;

impl InstEval for ListAppend {
    fn execute(&self, ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let lock = ctxt.get_env().lock_collect();
        let list_value = stack.pop(&lock)?;
        let list = list_value.as_list()?;
        let value = stack.pop(&lock)?;
        let list = list.borrow();
        list.append(value);
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
