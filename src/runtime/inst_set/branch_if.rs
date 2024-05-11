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
pub struct BranchIf(BranchTarget);

impl BranchIf {
    pub fn new(index: BranchTarget) -> Self {
        BranchIf(index)
    }
}

impl InstEval for BranchIf {
    fn execute(&self, ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        let lock = ctxt.get_env().lock_collect();
        let cond = stack.pop(&lock)?.as_bool()?;
        Ok(if cond {
            InstructionResult::Next(InstructionTarget::Branch(self.0.target_index()))
        } else {
            InstructionResult::Next(InstructionTarget::Step)
        })
    }
}
