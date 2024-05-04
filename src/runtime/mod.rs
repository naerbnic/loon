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
        let mut args = Vec::with_capacity(num_args as usize);
        for _ in 0..num_args {
            args.push(self.stack.pop()?);
        }
        args.reverse();
        let mut eval_context = EvalContext::new(self, function.as_function()?.clone(), args)?;
        let mut returned_values = eval_context.run()?;
        let num_returned = returned_values.len() as u32;
        for returned_value in returned_values.drain(..) {
            self.stack.push(returned_value);
        }
        Ok(num_returned)
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
    pub fn new(
        top_level: &'a mut TopLevelRuntime,
        function: GcRef<Function>,
        args: Vec<Value>,
    ) -> Result<Self> {
        let stack_frame = function.with_mut(|func| func.make_stack_frame(args))?;
        Ok(EvalContext {
            top_level,
            call_stack: vec![stack_frame],
        })
    }

    fn run(&mut self) -> Result<Vec<Value>> {
        loop {
            let frame = self.call_stack.last_mut().unwrap();
            match frame.run_to_frame_change(&self.top_level.global_context)? {
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
                    let function = call.function;
                    let args = call.args;
                    let stack_frame = function.with_mut(|f| f.make_stack_frame(args))?;
                    self.call_stack.push(stack_frame);
                }
            }
        }
    }
}
