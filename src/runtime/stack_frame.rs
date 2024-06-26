use std::{cell::RefCell, rc::Rc};

use crate::{
    binary::{instructions::StackIndex, modules::ImportSource},
    gc::{GcRef, GcRefVisitor, GcTraceable, PinnedGcRef},
    pure_values::{Float, Integer},
    runtime::value::NativeFunctionResult,
    util::imm_string::ImmString,
};

use super::{
    constants::ValueTable,
    context::InstEvalContext,
    error::{Result, RuntimeError},
    global_env::GlobalEnv,
    instructions::{
        CallStepResult, FrameChange, InstEval, InstEvalList, InstructionResult, InstructionTarget,
        YieldStepResult,
    },
    modules::ModuleGlobals,
    value::{
        Function, List, NativeFunctionContext, NativeFunctionPtr, NativeFunctionResultInner,
        PinnedValue, Value,
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

impl GcTraceable for InstState {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        self.inst_list.trace(visitor);
    }
}

pub(crate) type PinnedValueBuffer = Vec<PinnedValue>;

pub(crate) struct LocalStack {
    stack: RefCell<Vec<Value>>,
}

impl LocalStack {
    pub fn new(env: &GlobalEnv) -> PinnedGcRef<Self> {
        env.create_pinned_ref(LocalStack {
            stack: RefCell::new(Vec::new()),
        })
    }

    pub fn push(&self, value: PinnedValue) {
        self.stack.borrow_mut().push(value.to_value());
    }

    pub fn pop(&self) -> Result<PinnedValue> {
        self.stack
            .borrow_mut()
            .pop()
            .map(Value::into_pinned)
            .ok_or_else(|| RuntimeError::new_operation_precondition_error("Local stack is empty."))
    }

    pub fn pop_n(&self, n: usize) -> Result<()> {
        let mut stack = self.stack.borrow_mut();
        let trunc_len = stack.len().checked_sub(n).ok_or_else(|| {
            RuntimeError::new_operation_precondition_error("Local stack is too small.")
        })?;
        stack.truncate(trunc_len);
        Ok(())
    }

    pub fn get_at_index(&self, index: StackIndex) -> Result<PinnedValue> {
        let index = match index {
            StackIndex::FromTop(i) => self
                .stack
                .borrow()
                .len()
                .checked_sub((i as usize) + 1)
                .ok_or_else(|| RuntimeError::new_internal_error("Stack index out of range"))?,
            StackIndex::FromBottom(i) => i as usize,
        };
        self.stack
            .borrow()
            .get(index)
            .ok_or_else(|| RuntimeError::new_internal_error("Stack index out of range."))
            .map(Value::pin)
    }

    pub fn set_at_index(&self, index: StackIndex, value: PinnedValue) -> Result<()> {
        let index = match index {
            StackIndex::FromTop(i) => self
                .stack
                .borrow()
                .len()
                .checked_sub((i as usize) + 1)
                .ok_or_else(|| RuntimeError::new_internal_error("Stack index out of range"))?,
            StackIndex::FromBottom(i) => i as usize,
        };
        self.stack.borrow_mut()[index] = value.to_value();
        Ok(())
    }

    pub fn drain_top_n(&self, len: u32, buffer: &mut PinnedValueBuffer) -> Result<()> {
        let len = len as usize;
        let mut src_stack = self.stack.borrow_mut();
        let start = src_stack.len().checked_sub(len).ok_or_else(|| {
            RuntimeError::new_operation_precondition_error("Local stack is too small.")
        })?;
        buffer.extend(src_stack[start..].iter().map(Value::pin));
        src_stack.truncate(start);
        Ok(())
    }

    pub fn push_iter(&self, env: &GlobalEnv, iter: impl Iterator<Item = PinnedValue>) {
        env.with_lock(|l| {
            self.stack
                .borrow_mut()
                .extend(iter.map(|v| v.into_value(l)))
        })
    }
}

impl GcTraceable for LocalStack {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        for value in self.stack.borrow().iter() {
            value.trace(visitor);
        }
    }
}

pub struct StackContext<'a> {
    env: &'a GlobalEnv,
    stack: PinnedGcRef<LocalStack>,
}

impl<'a> StackContext<'a> {
    pub(crate) fn new(env: &'a GlobalEnv, stack: PinnedGcRef<LocalStack>) -> Self {
        StackContext { env, stack }
    }
    pub fn push_import(&mut self, source: &ImportSource) -> Result<()> {
        let value = self.env.get_import(source)?;
        self.stack.push(value);
        Ok(())
    }

    pub fn push_bool(&mut self, value: bool) {
        self.stack.push(PinnedValue::new_bool(value));
    }

    pub fn push_int(&mut self, value: impl Into<Integer>) {
        self.stack.push(PinnedValue::new_integer(value.into()));
    }

    pub fn push_float(&mut self, value: f64) {
        self.stack.push(PinnedValue::new_float(value.into()));
    }

    pub fn push_string(&mut self, value: impl AsRef<str>) {
        self.stack
            .push(PinnedValue::new_string(ImmString::from_str(value.as_ref())));
    }

    pub fn make_list(&mut self, size: usize) -> Result<()> {
        let mut list = Vec::with_capacity(size);
        for _ in 0..size {
            list.push(self.stack.pop()?);
        }
        // FIXME: Which direction should the list be from the stack?
        // Current here is from first push to last.
        list.reverse();
        self.stack
            .push(PinnedValue::new_list(List::from_iter(self.env, list)));
        Ok(())
    }

    pub fn push_native_function<F>(&mut self, function: F)
    where
        F: Fn(NativeFunctionContext) -> Result<NativeFunctionResult> + 'static,
    {
        self.stack
            .push(PinnedValue::new_function(Function::new_native(
                self.env, function,
            )));
    }

    pub fn get_int(&self, index: StackIndex) -> Result<Integer> {
        Ok(self.stack.get_at_index(index)?.as_int()?.clone())
    }

    pub fn get_float(&self, index: StackIndex) -> Result<Float> {
        Ok(self.stack.get_at_index(index)?.as_float()?.clone())
    }

    pub fn get_bool(&self, index: StackIndex) -> Result<bool> {
        self.stack.get_at_index(index)?.as_bool()
    }

    pub fn get_string<F, R>(&self, index: StackIndex, body: F) -> Result<R>
    where
        F: FnOnce(&str) -> Result<R>,
    {
        body(self.stack.get_at_index(index)?.as_str()?)
    }

    pub fn pop_n(&mut self, n: usize) -> Result<()> {
        self.stack.pop_n(n)
    }
}

struct ManagedFrameState {
    inst_state: RefCell<InstState>,
    local_consts: GcRef<ValueTable>,
    module_globals: GcRef<ModuleGlobals>,
}

impl ManagedFrameState {
    pub fn step(
        &self,
        ctxt: &GlobalEnv,
        local_stack: &PinnedGcRef<LocalStack>,
    ) -> Result<Option<FrameChange>> {
        let local_consts = self.local_consts.pin();
        let globals = self.module_globals.pin();
        let inst_eval_ctxt = InstEvalContext::new(ctxt, &local_consts, &globals);
        let mut inst_state = self.inst_state.borrow_mut();
        let inst = inst_state.curr_inst();
        let result = match inst.execute(&inst_eval_ctxt, local_stack)? {
            InstructionResult::Next(target) => {
                inst_state.update_pc(target)?;
                None
            }
            InstructionResult::Return(num_values) => Some(FrameChange::Return(num_values)),
            InstructionResult::Call(func_call) => {
                inst_state.update_pc(func_call.return_target())?;
                let call = CallStepResult {
                    num_args: func_call.num_args(),
                };
                Some(FrameChange::Call(call))
            }
            InstructionResult::TailCall(func_call) => Some(FrameChange::TailCall(CallStepResult {
                num_args: func_call.num_args(),
            })),
        };
        Ok(result)
    }

    pub fn run_to_frame_change(
        &self,
        ctxt: &GlobalEnv,
        local_stack: &PinnedGcRef<LocalStack>,
    ) -> Result<FrameChange> {
        loop {
            if let Some(result) = self.step(ctxt, local_stack)? {
                return Ok(result);
            }
        }
    }
}

impl GcTraceable for ManagedFrameState {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        self.inst_state.borrow().trace(visitor);
        self.local_consts.trace(visitor);
        self.module_globals.trace(visitor);
    }
}

struct NativeFrameState {
    native_func: RefCell<NativeFunctionPtr>,
}

impl NativeFrameState {
    pub fn run_to_frame_change(
        &self,
        env: &GlobalEnv,
        local_stack: &PinnedGcRef<LocalStack>,
    ) -> Result<FrameChange> {
        let ctxt = NativeFunctionContext::new(env, local_stack);
        match self.native_func.borrow().call(ctxt)?.0 {
            NativeFunctionResultInner::ReturnValue(num_values) => {
                Ok(FrameChange::Return(num_values))
            }
            NativeFunctionResultInner::TailCall(tail_call) => {
                Ok(FrameChange::TailCall(CallStepResult {
                    num_args: tail_call.num_args,
                }))
            }
            NativeFunctionResultInner::CallWithContinuation(call) => {
                *self.native_func.borrow_mut() = call.continuation().clone();
                Ok(FrameChange::Call(CallStepResult {
                    num_args: call.num_args(),
                }))
            }
            NativeFunctionResultInner::YieldCall(_call) => {
                Ok(FrameChange::YieldCall(YieldStepResult))
            }
        }
    }
}

impl GcTraceable for NativeFrameState {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        self.native_func.borrow().trace(visitor);
    }
}

enum FrameState {
    Managed(ManagedFrameState),
    Native(NativeFrameState),
}

impl GcTraceable for FrameState {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        match self {
            FrameState::Managed(state) => state.trace(visitor),
            FrameState::Native(state) => state.trace(visitor),
        }
    }
}

pub struct StackFrame {
    frame_state: FrameState,
    local_stack: GcRef<LocalStack>,
}

impl StackFrame {
    pub fn new_managed(
        env: &GlobalEnv,
        inst_list: Rc<InstEvalList>,
        local_consts: PinnedGcRef<ValueTable>,
        module_globals: PinnedGcRef<ModuleGlobals>,
        local_stack: PinnedGcRef<LocalStack>,
    ) -> PinnedGcRef<Self> {
        env.with_lock(|lock| {
            env.create_pinned_ref(StackFrame {
                frame_state: FrameState::Managed(ManagedFrameState {
                    inst_state: RefCell::new(InstState::new(inst_list)),
                    local_consts: local_consts.into_ref(lock.guard()),
                    module_globals: module_globals.into_ref(lock.guard()),
                }),
                local_stack: local_stack.into_ref(lock.guard()),
            })
        })
    }

    pub fn new_native(
        env: &GlobalEnv,
        native_func: NativeFunctionPtr,
        local_stack: PinnedGcRef<LocalStack>,
    ) -> PinnedGcRef<Self> {
        env.with_lock(|lock| {
            env.create_pinned_ref(StackFrame {
                frame_state: FrameState::Native(NativeFrameState {
                    native_func: RefCell::new(native_func),
                }),
                local_stack: local_stack.into_ref(lock.guard()),
            })
        })
    }

    pub fn run_to_frame_change(&self, ctxt: &GlobalEnv) -> Result<FrameChange> {
        let local_stack = self.local_stack.pin();
        match &self.frame_state {
            FrameState::Managed(state) => state.run_to_frame_change(ctxt, &local_stack),
            FrameState::Native(state) => state.run_to_frame_change(ctxt, &local_stack),
        }
    }

    pub fn pop(&self) -> Result<PinnedValue> {
        self.local_stack.borrow().pop()
    }

    pub fn push_iter(&self, env: &GlobalEnv, iter: impl Iterator<Item = PinnedValue>) {
        self.local_stack.borrow().push_iter(env, iter);
    }

    pub fn drain_top_n(&self, len: u32, buffer: &mut PinnedValueBuffer) -> Result<()> {
        let src_stack = self.local_stack.borrow();
        src_stack.drain_top_n(len, buffer)
    }
}

impl GcTraceable for StackFrame {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        self.frame_state.trace(visitor);
        self.local_stack.trace(visitor);
    }
}
