use crate::{
    binary::instructions::CompareOp,
    runtime::{
        context::InstEvalContext,
        error::Result,
        instructions::{InstEval, InstructionResult, InstructionTarget},
        stack_frame::LocalStack,
        value::Value,
    },
};

#[derive(Clone, Debug)]
pub struct Compare(CompareOp);

impl Compare {
    pub fn new(cmp_op: CompareOp) -> Self {
        Compare(cmp_op)
    }
}

impl InstEval for Compare {
    fn execute(&self, _ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let right = stack.pop()?;
        let left = stack.pop()?;
        let result = match self.0 {
            CompareOp::RefEq => left.ref_eq(&right),
            CompareOp::Eq => todo!(),
            CompareOp::Ne => todo!(),
            CompareOp::Lt => todo!(),
            CompareOp::Le => todo!(),
            CompareOp::Gt => todo!(),
            CompareOp::Ge => todo!(),
        };
        stack.push(Value::Bool(result));
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
