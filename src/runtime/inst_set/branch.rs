use crate::{
    binary::instructions::BranchTarget,
    runtime::{
        context::InstEvalContext,
        error::Result,
        instructions::{InstEval, InstructionResult, InstructionTarget},
        stack_frame::LocalStack,
    },
};

#[derive(Clone, Debug)]
pub struct Branch(BranchTarget);

impl Branch {
    pub fn new(index: BranchTarget) -> Self {
        Branch(index)
    }
}

impl InstEval for Branch {
    fn execute(&self, _ctxt: &InstEvalContext, _stack: &LocalStack) -> Result<InstructionResult> {
        Ok(InstructionResult::Next(InstructionTarget::Branch(
            self.0.target_index(),
        )))
    }
}
