use std::{borrow::Cow, rc::Rc};

use super::{
    context::InstEvalContext,
    error::{Result, RuntimeError},
    instructions::{
        CallStepResult, FrameChange, InstEval, InstEvalList, InstructionResult, InstructionTarget,
    },
    value::Value,
};

struct InstState {
    pc: usize,
    inst_list: Rc<InstEvalList>,
}

impl InstState {
    pub fn new(inst_list: Rc<InstEvalList>) -> Self {
        InstState { pc: 0, inst_list }
    }

    pub fn curr_inst(&self) -> &dyn InstEval {
        self.inst_list.inst_at(self.pc).unwrap()
    }

    pub fn pc(&self) -> usize {
        self.pc
    }

    pub fn update_pc(&mut self, pc: InstructionTarget) -> Result<()> {
        let next_pc = match pc {
            InstructionTarget::Step => self.pc + 1,
            InstructionTarget::Branch(i) => i,
        };
        if next_pc >= self.inst_list.len() {
            return Err(RuntimeError::new_operation_precondition_error(
                "Instruction stepped out of bounds.",
            ));
        }
        self.pc = next_pc;
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

    pub fn pop(&mut self) -> Result<Value> {
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
    pub fn new(inst_list: Rc<InstEvalList>, args: Vec<Value>) -> Self {
        StackFrame {
            inst_state: InstState::new(inst_list),
            local_stack: LocalStack::from_args(args),
        }
    }

    pub fn read_args_from_stack(&mut self) -> Result<Vec<Value>> {
        let arg_count_value = self.local_stack.pop()?;
        let arg_count = arg_count_value.as_compact_integer()?;
        let mut args = Vec::new();
        for _ in 0..arg_count {
            args.push(self.local_stack.pop()?);
        }
        Ok(args)
    }

    pub fn step(&mut self, ctxt: &InstEvalContext) -> Result<Option<FrameChange>> {
        let inst = self.inst_state.curr_inst();
        let result = match inst.execute(ctxt, &mut self.local_stack)? {
            InstructionResult::Next(target) => {
                self.inst_state.update_pc(target)?;
                None
            }
            InstructionResult::Return => Some(FrameChange::Return(self.read_args_from_stack()?)),
            InstructionResult::Call(func_call) => {
                let function = self.local_stack.pop()?;
                let args = self.read_args_from_stack()?;
                self.inst_state.update_pc(func_call.return_target())?;
                let call = CallStepResult { function, args };
                Some(FrameChange::Call(call))
            }
        };
        Ok(result)
    }

    pub fn run_to_frame_change(&mut self, ctxt: &InstEvalContext) -> Result<FrameChange> {
        loop {
            if let Some(result) = self.step(ctxt)? {
                return Ok(result);
            }
        }
    }

    pub fn push_return_values(&mut self, args: Vec<Value>) -> Result<()> {
        for arg in args.into_iter().rev() {
            self.local_stack.push(arg);
        }
        Ok(())
    }
}
