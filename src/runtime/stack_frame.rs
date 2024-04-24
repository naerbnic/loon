use std::{borrow::Cow, rc::Rc};

use super::{
    error::RuntimeError,
    instructions::{CallStepResult, FrameChange, Instruction, InstructionList, InstructionResult}, value::Value,
};

struct InstState {
    pc: usize,
    inst_list: Rc<InstructionList>,
}

impl InstState {
    pub fn new(inst_list: Rc<InstructionList>) -> Self {
        InstState { pc: 0, inst_list }
    }

    pub fn curr_inst(&self) -> &dyn Instruction {
        self.inst_list.inst_at(self.pc).unwrap()
    }

    pub fn pc(&self) -> usize {
        self.pc
    }

    pub fn update_pc(&mut self, pc: usize) -> Result<(), RuntimeError> {
        if pc >= self.inst_list.len() {
            return Err(RuntimeError::new_operation_precondition_error(
                "Instruction stepped out of bounds.",
            ));
        }
        self.pc = pc;
        Ok(())
    }
}

pub(crate) struct LocalStack {
    stack: Vec<Value>,
}

impl LocalStack {
    pub fn new() -> Self {
        LocalStack { stack: Vec::new() }
    }

    pub fn from_args<'a>(args: impl Into<Cow<'a, [Value]>>) -> Self {
        LocalStack {
            stack: args.into().into_owned(),
        }
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    pub fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    pub fn pop(&mut self) -> Result<Value, RuntimeError> {
        self.stack
            .pop()
            .ok_or_else(|| RuntimeError::new_operation_precondition_error("Local stack is empty."))
    }
}

pub struct StackFrame {
    inst_state: InstState,
    local_stack: LocalStack,
}

impl StackFrame {
    pub fn new(inst_list: Rc<InstructionList>, args: Vec<Value>) -> Self {
        StackFrame {
            inst_state: InstState::new(inst_list),
            local_stack: LocalStack::from_args(args),
        }
    }

    pub fn read_args_from_stack(&mut self) -> Result<Vec<Value>, RuntimeError> {
        let arg_count_value = self.local_stack.pop()?;
        let arg_count = arg_count_value.as_compact_integer()?;
        let mut args = Vec::new();
        for _ in 0..arg_count {
            args.push(self.local_stack.pop()?);
        }
        Ok(args)
    }

    pub fn step(&mut self) -> Result<Option<FrameChange>, RuntimeError> {
        let inst = self.inst_state.curr_inst();
        let result = match inst.execute(&mut self.local_stack)? {
            InstructionResult::Next => {
                self.inst_state.update_pc(self.inst_state.pc() + 1)?;
                None
            }
            InstructionResult::Branch(i) => {
                self.inst_state.update_pc(i)?;
                None
            }
            InstructionResult::Return => Some(FrameChange::Return(self.read_args_from_stack()?)),
            InstructionResult::Call(i) => {
                let function = self.local_stack.pop()?;
                let args = self.read_args_from_stack()?;
                self.inst_state.update_pc(i)?;
                let call = CallStepResult { function, args };
                Some(FrameChange::Call(call))
            }
        };
        Ok(result)
    }

    pub fn run_to_frame_change(&mut self) -> Result<FrameChange, RuntimeError> {
        loop {
            if let Some(result) = self.step()? {
                return Ok(result);
            }
        }
    }

    pub fn push_return_values(&mut self, args: Vec<Value>) -> Result<(), RuntimeError> {
        for arg in args.into_iter().rev() {
            self.local_stack.push(arg);
        }
        Ok(())
    }
}
