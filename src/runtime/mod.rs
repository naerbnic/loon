use std::rc::Rc;

use self::{instructions::InstructionList, stack_frame::StackFrame, value::Value};

pub(super) mod value;
pub(super) mod const_table;
pub(super) mod constants;
pub(super) mod error;
pub(super) mod inst_set;
pub(super) mod instructions;
pub(super) mod stack_frame;
pub(super) mod environment;

pub type Result<T> = std::result::Result<T, error::RuntimeError>;

pub struct Runtime {
    call_stack: Vec<StackFrame>,
}

impl Runtime {
    pub fn new(inst: Rc<InstructionList>) -> Self {
        let initial_frame = StackFrame::new(inst, Vec::new());
        Runtime {
            call_stack: vec![initial_frame],
        }
    }

    fn run(&mut self) -> Result<Vec<Value>> {
        loop {
            let frame = self.call_stack.last_mut().unwrap();
            match frame.run_to_frame_change()? {
                instructions::FrameChange::Return(args) => {
                    self.call_stack.pop().expect("Call stack is empty.");
                    match self.call_stack.last_mut() {
                        Some(frame) => {
                            frame.push_return_values(args);
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
