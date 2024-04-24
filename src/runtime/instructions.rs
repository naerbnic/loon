use std::rc::Rc;

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
pub(crate) trait Instruction {
    fn execute(&self, stack: &mut LocalStack) -> Result<InstructionResult, RuntimeError>;
}

pub type InstPtr = Rc<dyn Instruction>;

pub struct InstructionList(Vec<InstPtr>);

impl InstructionList {
    pub fn inst_at(&self, index: usize) -> Option<&dyn Instruction> {
        self.0.get(index).map(|e| &**e)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl FromIterator<InstPtr> for InstructionList {
    fn from_iter<T: IntoIterator<Item = InstPtr>>(iter: T) -> Self {
        InstructionList(FromIterator::from_iter(iter))
    }
}

impl std::fmt::Debug for InstructionList {
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
