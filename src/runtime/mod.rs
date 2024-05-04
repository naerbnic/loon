use crate::{
    binary::{instructions::StackIndex, modules::ImportSource},
    pure_values::Integer,
    refs::GcRef,
    util::imm_string::ImmString,
};

use self::{
    context::GlobalEnv,
    error::Result,
    stack_frame::{LocalStack, StackFrame},
    value::{Function, List, Value},
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
    pub fn push_import(&mut self, source: &ImportSource) -> Result<()> {
        let value = self.global_context.get_import(source)?;
        self.stack.push(value);
        Ok(())
    }

    pub fn push_bool(&mut self, value: bool) {
        self.stack.push(Value::Bool(value));
    }

    pub fn push_int(&mut self, value: impl Into<Integer>) {
        self.stack.push(Value::Integer(value.into()));
    }

    pub fn push_float(&mut self, value: f64) {
        self.stack.push(Value::Float(value.into()));
    }

    pub fn push_string(&mut self, value: impl AsRef<str>) {
        self.stack
            .push(Value::String(ImmString::from_str(value.as_ref())));
    }

    pub fn make_list(&mut self, size: usize) -> Result<()> {
        let mut list = Vec::with_capacity(size);
        for _ in 0..size {
            list.push(self.stack.pop()?);
        }
        // FIXME: Which direction should the list be from the stack?
        // Current here is from first push to last.
        list.reverse();
        self.stack.push(Value::List(
            self.global_context.create_ref(List::from_iter(list)),
        ));
        Ok(())
    }

    pub fn pop_n(&mut self, n: usize) -> Result<()> {
        for _ in 0..n {
            self.stack.pop()?;
        }
        Ok(())
    }

    pub fn call_function(&mut self, num_args: u32) -> Result<u32> {
        let function = self.stack.pop()?;
        let mut eval_context = EvalContext::new(self);
        eval_context.run(function.as_function()?.clone(), num_args)
    }

    pub fn get_int(&self, index: StackIndex) -> Result<Integer> {
        Ok(self.stack.get_at_index(index)?.as_int()?.clone())
    }
}

struct EvalContext<'a> {
    top_level: &'a mut TopLevelRuntime,
    call_stack: Vec<StackFrame>,
}

impl<'a> EvalContext<'a> {
    pub fn new(top_level: &'a mut TopLevelRuntime) -> Self {
        EvalContext {
            top_level,
            call_stack: Vec::new(),
        }
    }

    fn run(&mut self, function: GcRef<Function>, num_args: u32) -> Result<u32> {
        let stack_frame = function
            .with_mut(|func| func.make_stack_frame(self.top_level.stack.drain_top_n(num_args)?))?;
        self.call_stack.push(stack_frame);
        loop {
            let frame = self.call_stack.last_mut().unwrap();
            match frame.run_to_frame_change(&self.top_level.global_context)? {
                instructions::FrameChange::Return(num_returns) => {
                    let mut prev_frame = self.call_stack.pop().expect("Call stack is empty.");
                    match self.call_stack.last_mut() {
                        Some(frame) => {
                            frame.push_iter(prev_frame.drain_top_n(num_returns)?);
                        }
                        None => {
                            self.top_level
                                .stack
                                .push_iter(prev_frame.drain_top_n(num_returns)?);
                            return Ok(num_returns);
                        }
                    }
                }
                instructions::FrameChange::Call(call) => {
                    let function = call.function;
                    let args = frame.drain_top_n(call.num_args)?;
                    let stack_frame = function.with_mut(|f| f.make_stack_frame(args))?;
                    self.call_stack.push(stack_frame);
                }
            }
        }
    }
}
