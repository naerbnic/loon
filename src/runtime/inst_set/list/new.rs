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
        let list = Value::List(ctxt.get_env().create_ref(List::new()));
        stack.push(list);
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
