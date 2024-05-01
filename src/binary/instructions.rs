use std::{
    borrow::{Borrow, Cow},
    collections::HashSet,
    rc::Rc,
};

use crate::util::{imm_string::ImmString, intern::InternSet};

/// An opcode for an instruction.
///
/// We're following the WASM idea for opcodes, where the opcodes are actually
/// full on identifiers. In the binary file, it can state a set of opcodes,
/// and mappings from integers to opcodes for the encoding to a file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Opcode(ImmString);

#[derive(Clone, Debug)]
pub enum InstArg {
    Integer(u32),
}

#[derive(Copy, Clone, Debug)]
pub enum StackIndex {
    FromTop(u32),
    FromBottom(u32),
}

#[derive(Copy, Clone, Debug)]
pub struct BranchTarget(u32);

#[derive(Copy, Clone, Debug)]
pub enum CompareOp {
    // Referential equality.
    RefEq,

    // Value equality. May call a method on the value.
    // NOTE: Do we want to do this? This might block our method call semantics.
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Copy, Clone, Debug)]
pub struct CallInstruction {
    pub function: StackIndex,
    pub num_args: u32,
    pub num_returns: u32,
}

#[derive(Clone, Debug)]
pub struct OtherInstruction {
    opcode: Opcode,
    args: Vec<InstArg>,
}

#[derive(Clone, Debug)]
pub enum Instruction {
    /// Push a constant onto the stack.
    PushConst(u32),

    /// Push a copy of the given stack index onto the stack.
    PushCopy(StackIndex),

    /// Pop the top N values off of the stack.
    Pop(u32),

    /// Add the top two values on the stack. Push the result.
    Add,

    // Boolean Operations
    /// Boolean AND the top two values on the stack. Push the result.
    BoolAnd,
    BoolOr,
    BoolXor,
    BoolNot,

    /// Compare the top two values on the stack, applying the given comparison.
    Compare(CompareOp),

    /// Pop the top value off of the stack and branch if it is true. The value
    /// at the top of the stack must be a boolean.
    BranchIf(BranchTarget),

    /// Calls a function. The number of arguments and return values are given
    /// as enum parameters. If the function does not return the specified number
    /// of values, an error will occur.
    Call(CallInstruction),

    /// Call a function. The top of the stack must be the function value,
    /// followed by an integer representing the number of arguments, followed by
    /// the arguments. The value is the index of the instruction to return to.
    CallDynamic,

    /// Returns from a function. The parameter gives the number of return values
    /// that will be popped off of the stack.
    Return(u32),

    /// Returns from a function. The top of the stack must be an integer
    /// representing the number of return values, followed by the return values.
    ReturnDynamic,

    Other(OtherInstruction),
}

#[derive(Clone, Debug)]
pub struct InstructionList(Rc<Vec<Instruction>>);

pub struct InstructionListBuilder {
    interned_opcodes: InternSet<ImmString>,
    instructions: Vec<Instruction>,
}

macro_rules! inst_builder {
    ($name:ident, $opcode:ident $(($($arg_name:ident : $arg_type:ty)*))?) => {
        pub fn $name(mut self, $($($arg_name: $arg_type),*)*) -> Self {
            self.instructions.push(Instruction::$opcode$(($($arg_name),*))*);
            self
        }
    };
}

impl InstructionListBuilder {
    pub fn new() -> Self {
        InstructionListBuilder {
            interned_opcodes: InternSet::new(),
            instructions: Vec::new(),
        }
    }

    inst_builder!(push_const, PushConst(c: u32));
    inst_builder!(push_copy, PushCopy(s: StackIndex));
    inst_builder!(pop, Pop(n: u32));
    inst_builder!(add, Add);
    inst_builder!(bool_and, BoolAnd);
    inst_builder!(bool_or, BoolOr);
    inst_builder!(bool_xor, BoolXor);
    inst_builder!(bool_not, BoolNot);
    inst_builder!(compare, Compare(op: CompareOp));
    inst_builder!(branch_if, BranchIf(target: BranchTarget));
    inst_builder!(call, Call(call: CallInstruction));
    inst_builder!(call_dynamic, CallDynamic);
    inst_builder!(return_, Return(n: u32));
    inst_builder!(return_dynamic, ReturnDynamic);

    pub fn build(self) -> InstructionList {
        InstructionList(Rc::new(self.instructions))
    }
}

impl Default for InstructionListBuilder {
    fn default() -> Self {
        InstructionListBuilder::new()
    }
}

/// A module containing test helpers.
#[cfg(test)]
pub mod testing {}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn test_push_basic_instructions() {
        let _inst_list = InstructionListBuilder::new()
            // Pop arg count from stack.
            .pop(1)
            // Push the constant 0 and 1 onto the stack.
            .push_const(0)
            .push_const(1)
            .add()
            .return_(1)
            .build();
    }
}
