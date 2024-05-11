use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
    value::{List, Value},
};

#[derive(Clone, Debug)]
pub struct ListNew;

impl InstEval for ListNew {
    fn execute(&self, ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let env_lock = ctxt.get_env().lock_collect();
        let list = Value::new_list(env_lock.create_ref(List::new()));
        stack.push(list);
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
