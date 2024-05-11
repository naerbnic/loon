use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
    value::Value,
};

#[derive(Clone, Debug)]
pub struct BoolNot;

impl InstEval for BoolNot {
    fn execute(&self, ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let lock = ctxt.get_env().lock_collect();
        let b1 = stack.pop(&lock)?.as_bool()?;
        stack.push(Value::new_bool(!b1));
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
