use std::rc::Rc;

use crate::binary::instructions::InstructionList;

use super::{error::RuntimeError, stack_frame::LocalStack, value::Value};

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

/// An object that can be executed as an instruction.
///
/// These are reused across multiple stack frames, so they should be immutable.
/// Further, as they will likely be shared across multiple contexts, they should
/// not contain any references to `loon::Value` objects.
pub(crate) trait InstEval {
    fn execute(&self, stack: &mut LocalStack) -> Result<InstructionResult, RuntimeError>;
}

pub type InstPtr = Rc<dyn InstEval>;

pub(crate) struct InstEvalList(Vec<InstPtr>);

impl InstEvalList {
    pub fn from_inst_list(inst_list: &InstructionList) -> Self {
        todo!()
    }

    pub fn inst_at(&self, index: usize) -> Option<&dyn InstEval> {
        self.0.get(index).map(|e| &**e)
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

impl std::fmt::Debug for InstEvalList {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_tuple("InstructionList")
            .field(&self.0.len())
            .finish()
    }
}

pub struct CallStepResult {
    pub function: Value,
    pub args: Vec<Value>,
}

pub enum FrameChange {
    Return(Vec<Value>),
    Call(CallStepResult),
}
