use crate::runtime::{
    context::InstEvalContext,
    instructions::{InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
    Result,
};

#[derive(Clone, Debug)]
pub struct BindFront(u32);

impl BindFront {
    pub fn new(num_args: u32) -> Self {
        BindFront(num_args)
    }
}

impl InstEval for BindFront {
    fn execute(&self, ctxt: &InstEvalContext, stack: &LocalStack) -> Result<InstructionResult> {
        ctxt.get_env().with_value_buffer(|buffer| {
            stack.drain_top_n(self.0, buffer)?;
            let func = stack.pop()?.as_function()?.clone();
            let new_func = func.bind_front(ctxt.get_env(), &func, buffer);
            stack.push(new_func.into());
            Ok(InstructionResult::Next(InstructionTarget::Step))
        })
    }
}
