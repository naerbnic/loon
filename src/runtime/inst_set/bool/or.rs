use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
    value::Value,
};

#[derive(Clone, Debug)]
pub struct BoolOr;

impl InstEval for BoolOr {
    fn execute(&self, ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let lock = ctxt.get_env().lock_collect();
        let b1 = stack.pop(&lock)?.as_bool()?;
        let b2 = stack.pop(&lock)?.as_bool()?;
        stack.push(Value::new_bool(b1 || b2));
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
