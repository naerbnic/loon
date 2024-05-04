use std::rc::Rc;

use crate::binary::instructions::StackIndex;

use super::{
    constants::ValueTable,
    context::{GlobalEnv, InstEvalContext},
    error::{Result, RuntimeError},
    instructions::{
        CallStepResult, FrameChange, InstEval, InstEvalList, InstructionResult, InstructionTarget,
    },
    modules::ModuleGlobals,
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

    pub fn update_pc(&mut self, pc: InstructionTarget) -> Result<()> {
        let next_pc = match pc {
            InstructionTarget::Step => self.pc + 1,
            InstructionTarget::Branch(i) => usize::try_from(i).unwrap(),
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

    pub fn from_args(args: impl IntoIterator<Item = Value>) -> Self {
        LocalStack {
            stack: args.into_iter().collect(),
        }
    }

    pub fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    pub fn pop(&mut self) -> Result<Value> {
        self.stack
            .pop()
            .ok_or_else(|| RuntimeError::new_operation_precondition_error("Local stack is empty."))
    }

    pub fn get_at_index(&self, index: StackIndex) -> Result<&Value> {
        let index = match index {
            StackIndex::FromTop(i) => self
                .stack
                .len()
                .checked_sub((i as usize) + 1)
                .ok_or_else(|| RuntimeError::new_internal_error("Stack index out of range"))?,
            StackIndex::FromBottom(i) => i as usize,
        };
        self.stack
            .get(index)
            .ok_or_else(|| RuntimeError::new_internal_error("Stack index out of range."))
    }

    pub fn drain_top_n(&mut self, len: u32) -> Result<impl Iterator<Item = Value> + '_> {
        let len = len as usize;
        let start = self.stack.len().checked_sub(len).ok_or_else(|| {
            RuntimeError::new_operation_precondition_error("Local stack is too small.")
        })?;
        Ok(self.stack.drain(start..))
    }

    pub fn push_iter(&mut self, iter: impl IntoIterator<Item = Value>) {
        self.stack.extend(iter);
    }
}

struct ManagedFrameState {
    inst_state: InstState,
    local_consts: ValueTable,
    module_globals: ModuleGlobals,
}

impl ManagedFrameState {
    pub fn step(
        &mut self,
        ctxt: &GlobalEnv,
        local_stack: &mut LocalStack,
    ) -> Result<Option<FrameChange>> {
        let inst_eval_ctxt = InstEvalContext::new(ctxt, &self.local_consts, &self.module_globals);
        let inst = self.inst_state.curr_inst();
        let result = match inst.execute(&inst_eval_ctxt, local_stack)? {
            InstructionResult::Next(target) => {
                self.inst_state.update_pc(target)?;
                None
            }
            InstructionResult::Return(num_values) => Some(FrameChange::Return(num_values)),
            InstructionResult::Call(func_call) => {
                let function = func_call.function().clone();
                self.inst_state.update_pc(func_call.return_target())?;
                let call = CallStepResult {
                    function,
                    num_args: func_call.num_args(),
                };
                Some(FrameChange::Call(call))
            }
        };
        Ok(result)
    }

    pub fn run_to_frame_change(
        &mut self,
        ctxt: &GlobalEnv,
        local_stack: &mut LocalStack,
    ) -> Result<FrameChange> {
        loop {
            if let Some(result) = self.step(ctxt, local_stack)? {
                return Ok(result);
            }
        }
    }
}

struct NativeFrameState {}

enum FrameState {
    Managed(ManagedFrameState),
    Native(NativeFrameState),
}

pub struct StackFrame {
    frame_state: FrameState,
    local_stack: LocalStack,
}

impl StackFrame {
    pub fn new(
        inst_list: Rc<InstEvalList>,
        local_consts: ValueTable,
        module_globals: ModuleGlobals,
        args: impl IntoIterator<Item = Value>,
    ) -> Self {
        StackFrame {
            frame_state: FrameState::Managed(ManagedFrameState {
                inst_state: InstState::new(inst_list),
                local_consts,
                module_globals,
            }),
            local_stack: LocalStack::from_args(args),
        }
    }

    pub fn run_to_frame_change(&mut self, ctxt: &GlobalEnv) -> Result<FrameChange> {
        match &mut self.frame_state {
            FrameState::Managed(state) => state.run_to_frame_change(ctxt, &mut self.local_stack),
            FrameState::Native(_) => todo!(),
        }
    }

    pub fn push_iter(&mut self, args: impl Iterator<Item = Value>) {
        self.local_stack.push_iter(args);
    }

    pub fn drain_top_n(&mut self, len: u32) -> Result<impl Iterator<Item = Value> + '_> {
        self.local_stack.drain_top_n(len)
    }
}
