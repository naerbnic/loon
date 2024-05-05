use self::{
    context::GlobalEnv,
    error::Result,
    stack_frame::{LocalStack, StackContext, StackFrame},
    value::Function,
};

pub(super) mod constants;
pub(super) mod context;
pub(super) mod environment;
pub(super) mod error;
pub(super) mod inst_set;
pub(super) mod instructions;
pub(super) mod modules;
pub(super) mod stack_frame;
pub(super) mod value;

pub struct TopLevelRuntime {
    global_context: GlobalEnv,
    stack: LocalStack,
}

impl TopLevelRuntime {
    pub fn new(global_context: GlobalEnv) -> Self {
        TopLevelRuntime {
            global_context,
            stack: LocalStack::new(),
        }
    }

    pub fn stack(&mut self) -> StackContext {
        StackContext::new(&self.global_context, &mut self.stack)
    }

    pub fn call_function(&mut self, num_args: u32) -> Result<u32> {
        let function = self.stack.pop()?;
        let mut eval_context = EvalContext::new(&self.global_context, &mut self.stack);
        eval_context.run(function.as_function()?.clone(), num_args)
    }
}

struct EvalContext<'a> {
    global_context: &'a GlobalEnv,
    parent_stack: &'a mut LocalStack,
    call_stack: Vec<StackFrame>,
}

impl<'a> EvalContext<'a> {
    pub fn new(global_context: &'a GlobalEnv, parent_stack: &'a mut LocalStack) -> Self {
        EvalContext {
            global_context,
            parent_stack,
            call_stack: Vec::new(),
        }
    }

    fn run(&mut self, function: Function, num_args: u32) -> Result<u32> {
        let stack_frame = function
            .make_stack_frame(self.parent_stack.drain_top_n(num_args)?, LocalStack::new())?;
        self.call_stack.push(stack_frame);
        loop {
            let frame = self.call_stack.last_mut().unwrap();
            match frame.run_to_frame_change(self.global_context)? {
                instructions::FrameChange::Return(num_returns) => {
                    let mut prev_frame = self.call_stack.pop().expect("Call stack is empty.");
                    match self.call_stack.last_mut() {
                        Some(frame) => {
                            frame.push_iter(prev_frame.drain_top_n(num_returns)?);
                        }
                        None => {
                            self.parent_stack
                                .push_iter(prev_frame.drain_top_n(num_returns)?);
                            return Ok(num_returns);
                        }
                    }
                }
                instructions::FrameChange::Call(call) => {
                    let function = call.function;
                    let args = frame.drain_top_n(call.num_args)?;
                    let stack_frame = function.make_stack_frame(args, LocalStack::new())?;
                    self.call_stack.push(stack_frame);
                }
            }
        }
    }
}
