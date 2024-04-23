use std::rc::Rc;

use crate::Value;

use super::{error::RuntimeError, local_stack::LocalStack};

pub enum InstructionResult {
    /// Go to the next instruction.
    Next,

    /// Go to the instruction at the given index.
    Branch(usize),

    /// Return from the current function. The top of the stack must be an
    /// integer representing the number of return values, followed by the
    /// return values.
    Return,

    /// Call a function. The top of the stack must be the function value,
    /// followed by an integer representing the number of arguments, followed by
    /// the arguments. The value is the index of the instruction to return to.
    Call(usize),
}

pub(crate) trait Instruction {
    fn execute(&self, stack: &mut LocalStack) -> Result<InstructionResult, RuntimeError>;
}

pub type InstPtr = Rc<dyn Instruction>;

pub struct InstructionList(Vec<InstPtr>);

impl FromIterator<InstPtr> for InstructionList {
    fn from_iter<T: IntoIterator<Item = InstPtr>>(iter: T) -> Self {
        InstructionList(FromIterator::from_iter(iter))
    }
}

pub struct CallStepResult {
    pub function: Value,
    pub args: Vec<Value>,
}

struct InstState {
    pc: usize,
    inst_list: Rc<InstructionList>,
}

impl InstState {
    pub fn new(inst_list: Rc<InstructionList>) -> Self {
        InstState { pc: 0, inst_list }
    }

    pub fn curr_inst(&self) -> &dyn Instruction {
        &*self.inst_list.0[self.pc]
    }

    pub fn pc(&self) -> usize {
        self.pc
    }

    pub fn update_pc(&mut self, pc: usize) -> Result<(), RuntimeError> {
        if pc >= self.inst_list.0.len() {
            return Err(RuntimeError::new_operation_precondition_error(
                "Instruction stepped out of bounds.",
            ));
        }
        self.pc = pc;
        Ok(())
    }
}

pub enum FrameChange {
    Return(Vec<Value>),
    Call(CallStepResult),
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
