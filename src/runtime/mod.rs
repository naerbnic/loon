use self::{context::GlobalContext, error::Result, stack_frame::StackFrame, value::Value};

pub(super) mod constants;
pub(super) mod context;
pub(super) mod environment;
pub(super) mod error;
pub(super) mod inst_set;
pub(super) mod instructions;
pub(super) mod stack_frame;
pub(super) mod value;
pub(super) mod modules;

pub struct Runtime {
    global_context: GlobalContext,
    call_stack: Vec<StackFrame>,
}

impl Runtime {
    fn new_with_initial_stack_frame(
        global_context: GlobalContext,
        stack_frame: StackFrame,
    ) -> Self {
        Runtime {
            global_context: GlobalContext::new(),
            call_stack: vec![stack_frame],
        }
    }

    pub fn run(&mut self) -> Result<Vec<Value>> {
        loop {
            let frame = self.call_stack.last_mut().unwrap();
            match frame.run_to_frame_change(todo!())? {
                instructions::FrameChange::Return(args) => {
                    self.call_stack.pop().expect("Call stack is empty.");
                    match self.call_stack.last_mut() {
                        Some(frame) => {
                            frame.push_return_values(args)?;
                        }
                        None => return Ok(args),
                    }
                }
                instructions::FrameChange::Call(call) => {
                    let function = call.function.as_function()?;
                    let args = call.args;
                    let stack_frame = function.with_mut(|f| f.make_stack_frame(args))?;
                    self.call_stack.push(stack_frame);
                }
            }
        }
    }
}
