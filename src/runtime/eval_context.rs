use super::{
    error::Result,
    global_env::GlobalEnv,
    instructions::FrameChange,
    stack_frame::{LocalStack, StackFrame},
    value::Function,
};

pub struct EvalContext<'a> {
    global_context: &'a GlobalEnv,
    parent_stack: &'a LocalStack,
    call_stack: Vec<StackFrame>,
}

impl<'a> EvalContext<'a> {
    pub fn new(global_context: &'a GlobalEnv, parent_stack: &'a LocalStack) -> Self {
        EvalContext {
            global_context,
            parent_stack,
            call_stack: Vec::new(),
        }
    }

    pub fn run(&mut self, function: Function, num_args: u32) -> Result<u32> {
        let stack_frame = function.make_stack_frame(self.parent_stack.drain_top_n(num_args)?)?;
        self.call_stack.push(stack_frame);
        loop {
            let frame = self.call_stack.last_mut().unwrap();
            match frame.run_to_frame_change(self.global_context)? {
                FrameChange::Return(num_returns) => {
                    let prev_frame = self.call_stack.pop().expect("Call stack is empty.");
                    match self.call_stack.last_mut() {
                        Some(frame) => {
                            frame.push_sequence(prev_frame.drain_top_n(num_returns)?);
                        }
                        None => {
                            self.parent_stack
                                .push_sequence(prev_frame.drain_top_n(num_returns)?);
                            return Ok(num_returns);
                        }
                    }
                }
                FrameChange::Call(call) => {
                    let function = call.function;
                    let args = frame.drain_top_n(call.num_args)?;
                    let stack_frame = function.make_stack_frame(args)?;
                    self.call_stack.push(stack_frame);
                }
            }
        }
    }
}
