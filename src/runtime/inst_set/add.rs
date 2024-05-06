use crate::runtime::{
    context::InstEvalContext,
    error::{Result, RuntimeError},
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
    value::Value,
};

#[derive(Clone, Debug)]
pub struct Add;

impl InstEval for Add {
    fn execute(&self, _ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let a = stack.pop()?;
        let b = stack.pop()?;
        // Right now, only implement integer addition.
        let result = match (a, b) {
            (Value::Integer(i1), Value::Integer(i2)) => Value::Integer(i1.add_owned(i2)),
            (Value::Float(f1), Value::Float(f2)) => Value::Float(f1.add_owned(f2)),
            (_, _) => {
                return Err(RuntimeError::new_type_error(
                    "Addition is only supported for integers and floats.",
                ))
            }
        };
        stack.push(result);
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
