use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
    value::Value,
};

#[derive(Clone, Debug)]
pub struct ListLen;

impl InstEval for ListLen {
    fn execute(&self, ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let lock = ctxt.get_env().lock_collect();
        let list_value = stack.pop(&lock)?;
        let list = list_value.as_list()?.borrow();
        let len = list.len();
        stack.push(Value::new_integer(i64::try_from(len).unwrap().into()));
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
