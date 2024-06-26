use std::rc::Rc;

use crate::gc::{GcRefVisitor, GcTraceable};

use super::{context::InstEvalContext, error::RuntimeError, stack_frame::LocalStack};

#[derive(Clone, Copy, Debug)]
pub enum InstructionTarget {
    Step,
    Branch(u32),
}

pub struct FunctionCallResult {
    num_args: u32,
    return_target: InstructionTarget,
}

impl FunctionCallResult {
    pub fn new(num_args: u32, return_target: InstructionTarget) -> Self {
        FunctionCallResult {
            num_args,
            return_target,
        }
    }

    pub fn num_args(&self) -> u32 {
        self.num_args
    }

    pub fn return_target(&self) -> InstructionTarget {
        self.return_target
    }
}

pub enum InstructionResult {
    /// Go to the next instruction.
    Next(InstructionTarget),

    /// Return from the current function. The parameter is the number of values
    /// on the top of the stack to return.
    Return(u32),

    /// Call a function. The top of the stack must be the function value,
    /// followed by an integer representing the number of arguments, followed by
    /// the arguments. The value is the index of the instruction to return to.
    Call(FunctionCallResult),

    /// Call a function in tail position, returning from the current function
    /// with the results of the called function.
    TailCall(FunctionCallResult),
}

/// An object that can be executed as an instruction.
///
/// These are reused across multiple stack frames, so they should be immutable.
/// Further, as they will likely be shared across multiple contexts, they should
/// not contain any references to `loon::Value` objects.
pub(crate) trait InstEval: std::fmt::Debug {
    fn execute(
        &self,
        ctxt: &InstEvalContext,
        stack: &LocalStack,
    ) -> Result<InstructionResult, RuntimeError>;
}

#[derive(Clone, Debug)]
pub(crate) struct InstPtr(Rc<dyn InstEval>);

impl InstPtr {
    pub fn new<T>(inst: T) -> Self
    where
        T: InstEval + 'static,
    {
        InstPtr(Rc::new(inst))
    }

    pub fn to_eval(&self) -> &dyn InstEval {
        &*self.0
    }
}

#[derive(Clone, Debug)]
pub(crate) struct InstEvalList(Vec<InstPtr>);

impl InstEvalList {
    pub fn from_inst_ptrs(inst_list: Vec<InstPtr>) -> Self {
        InstEvalList(inst_list)
    }

    pub fn inst_at(&self, index: usize) -> Option<&dyn InstEval> {
        self.0.get(index).map(InstPtr::to_eval)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl FromIterator<InstPtr> for InstEvalList {
    fn from_iter<T: IntoIterator<Item = InstPtr>>(iter: T) -> Self {
        InstEvalList(FromIterator::from_iter(iter))
    }
}

impl GcTraceable for InstEvalList {
    fn trace<V>(&self, _visitor: &mut V)
    where
        V: GcRefVisitor,
    {
    }
}

pub struct CallStepResult {
    pub num_args: u32,
}

pub struct YieldStepResult;

pub enum FrameChange {
    Return(u32),
    Call(CallStepResult),
    TailCall(CallStepResult),
    YieldCall(YieldStepResult),
}
