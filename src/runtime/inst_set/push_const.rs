use crate::runtime::{
    context::InstEvalContext,
    error::Result,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
};

#[derive(Clone, Debug)]
pub struct PushConst(u32);

impl PushConst {
    pub fn new(index: u32) -> Self {
        PushConst(index)
    }
}

impl InstEval for PushConst {
    fn execute(&self, ctxt: &InstEvalContext, stack: &mut LocalStack) -> Result<InstructionResult> {
        let value = ctxt.get_constant(self.0)?;
        stack.push(value);
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
