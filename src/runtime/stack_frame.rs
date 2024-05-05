use std::rc::Rc;

use crate::{
    binary::{instructions::StackIndex, modules::ImportSource},
    pure_values::Integer,
    runtime::value::NativeFunctionResult,
    util::imm_string::ImmString,
};

use super::{
    constants::ValueTable,
    context::{GlobalEnv, InstEvalContext},
    error::{Result, RuntimeError},
    instructions::{
        CallStepResult, FrameChange, InstEval, InstEvalList, InstructionResult, InstructionTarget,
    },
    modules::ModuleGlobals,
    value::{
        Function, List, NativeFunctionContext, NativeFunctionPtr, NativeFunctionResultInner, Value,
    },
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

pub struct StackContext<'a> {
    global_context: &'a GlobalEnv,
    stack: &'a mut LocalStack,
}

impl<'a> StackContext<'a> {
    pub(crate) fn new(global_context: &'a GlobalEnv, stack: &'a mut LocalStack) -> Self {
        StackContext {
            global_context,
            stack,
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

    pub fn make_closure(&mut self, num_args: u32) -> Result<()> {
        let function = self.stack.pop()?.as_function()?.clone();
        let captured_values = self.stack.drain_top_n(num_args)?;
        let new_value = Value::Function(function.bind_front(
            self.global_context,
            captured_values,
        ));
        self.stack.push(new_value);
        Ok(())
    }

    pub fn push_native_function<F>(&mut self, function: F)
    where
        F: Fn(NativeFunctionContext) -> Result<NativeFunctionResult> + 'static,
    {
        self.stack.push(Value::Function(Function::new_native(
            self.global_context,
            function,
        )));
    }

    pub fn get_int(&self, index: StackIndex) -> Result<Integer> {
        Ok(self.stack.get_at_index(index)?.as_int()?.clone())
    }

    pub fn pop_n(&mut self, n: usize) -> Result<()> {
        for _ in 0..n {
            self.stack.pop()?;
        }
        Ok(())
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

struct NativeFrameState {
    native_func: NativeFunctionPtr,
}

impl NativeFrameState {
    pub fn run_to_frame_change(
        &mut self,
        ctxt: &GlobalEnv,
        local_stack: &mut LocalStack,
    ) -> Result<FrameChange> {
        let ctxt = NativeFunctionContext::new(ctxt, local_stack);
        match self.native_func.call(ctxt)?.0 {
            NativeFunctionResultInner::ReturnValue(num_values) => {
                Ok(FrameChange::Return(num_values))
            }
            NativeFunctionResultInner::TailCall(_) => todo!(),
            NativeFunctionResultInner::CallWithContinuation(call) => {
                self.native_func = call.continuation().clone();
                Ok(FrameChange::Call(CallStepResult {
                    function: call.function().clone(),
                    num_args: call.num_args(),
                }))
            }
        }
    }
}

enum FrameState {
    Managed(ManagedFrameState),
    Native(NativeFrameState),
}

pub struct StackFrame {
    frame_state: FrameState,
    local_stack: LocalStack,
}

impl StackFrame {
    pub fn new_managed(
        inst_list: Rc<InstEvalList>,
        local_consts: ValueTable,
        module_globals: ModuleGlobals,
        local_stack: LocalStack,
    ) -> Self {
        StackFrame {
            frame_state: FrameState::Managed(ManagedFrameState {
                inst_state: InstState::new(inst_list),
                local_consts,
                module_globals,
            }),
            local_stack,
        }
    }

    pub fn new_native(native_func: NativeFunctionPtr, local_stack: LocalStack) -> Self {
        StackFrame {
            frame_state: FrameState::Native(NativeFrameState { native_func }),
            local_stack,
        }
    }

    pub fn run_to_frame_change(&mut self, ctxt: &GlobalEnv) -> Result<FrameChange> {
        match &mut self.frame_state {
            FrameState::Managed(state) => state.run_to_frame_change(ctxt, &mut self.local_stack),
            FrameState::Native(state) => state.run_to_frame_change(ctxt, &mut self.local_stack),
        }
    }

    pub fn push_iter(&mut self, args: impl Iterator<Item = Value>) {
        self.local_stack.push_iter(args);
    }

    pub fn drain_top_n(&mut self, len: u32) -> Result<impl Iterator<Item = Value> + '_> {
        self.local_stack.drain_top_n(len)
    }
}
