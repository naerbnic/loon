use std::{collections::HashMap, rc::Rc};

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

impl BranchTarget {
    pub fn target_index(&self) -> u32 {
        self.0
    }
}

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
pub enum Instruction {
    /// Push a local constant onto the stack.
    PushConst(u32),

    /// Push a copy of the given stack index onto the stack.
    PushCopy(StackIndex),

    /// Push a module import onto the stack.
    PushImport(u32),

    /// Pushes the value of a global from the module.
    PushGlobal(u32),

    /// Pops the top value off of the stack and writes it to the global.
    PopGlobal(u32),

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
}

#[derive(Clone, Debug)]
pub struct InstructionList(Rc<Vec<Instruction>>);

pub struct InstructionListBuilder {
    branch_target_names: InternSet<ImmString>,
    branch_targets: HashMap<ImmString, BranchTarget>,
    branch_resolutions: Vec<(u32, ImmString)>,
    instructions: Vec<Option<Instruction>>,
}

macro_rules! inst_builder {
    ($name:ident, $opcode:ident $(($($arg_name:ident : $arg_type:ty)*))?) => {
        pub fn $name(&mut self, $($($arg_name: $arg_type),*)*) -> &mut Self {
            self.instructions.push(Some(Instruction::$opcode$(($($arg_name),*))*));
            self
        }
    };
}

impl InstructionListBuilder {
    pub fn new() -> Self {
        InstructionListBuilder {
            branch_target_names: InternSet::new(),
            branch_targets: HashMap::new(),
            branch_resolutions: Vec::new(),
            instructions: Vec::new(),
        }
    }

    inst_builder!(push_const, PushConst(c: u32));
    inst_builder!(push_import, PushImport(index: u32));
    inst_builder!(push_copy, PushCopy(s: StackIndex));
    inst_builder!(push_global, PushGlobal(index: u32));
    inst_builder!(pop_global, PopGlobal(index: u32));
    inst_builder!(pop, Pop(n: u32));
    inst_builder!(add, Add);
    inst_builder!(bool_and, BoolAnd);
    inst_builder!(bool_or, BoolOr);
    inst_builder!(bool_xor, BoolXor);
    inst_builder!(bool_not, BoolNot);
    inst_builder!(compare, Compare(op: CompareOp));
    inst_builder!(call, Call(call: CallInstruction));
    inst_builder!(call_dynamic, CallDynamic);
    inst_builder!(return_, Return(n: u32));
    inst_builder!(return_dynamic, ReturnDynamic);

    pub fn branch_if(&mut self, target: &str) -> &mut Self {
        let target = self.branch_target_names.intern(target);
        self.branch_resolutions
            .push((self.instructions.len() as u32, target));
        self.instructions.push(None);
        self
    }

    pub fn define_branch_target(&mut self, target: &str) -> &mut Self {
        let target = self.branch_target_names.intern(target);
        let curr_branch_target = BranchTarget(self.instructions.len() as u32);
        let result = self.branch_targets.insert(target, curr_branch_target);
        assert!(result.is_none());
        self
    }

    pub fn build(mut self) -> InstructionList {
        // Resolve branch targets.
        for (index, target) in self.branch_resolutions {
            let target = self.branch_targets.get(&target).unwrap();
            let prev = self.instructions[index as usize].replace(Instruction::BranchIf(*target));
            assert!(prev.is_none())
        }
        let result: Result<Vec<Instruction>, ()> =
            self.instructions.into_iter().map(|i| i.ok_or(())).collect();
        InstructionList(Rc::new(result.unwrap()))
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
    use super::*;

    #[test]
    fn test_push_basic_instructions() {
        let mut builder = InstructionListBuilder::new();
        // Pop arg count from stack.
        builder
            .pop(1)
            // Push the constant 0 and 1 onto the stack.
            .push_const(0)
            .push_const(1)
            .add()
            .return_(1);

        builder.build();
    }

    #[test]
    fn test_branch() {
        let mut builder = InstructionListBuilder::new();
        builder
            .define_branch_target("loop_start")
            .push_const(0)
            .branch_if("loop_start");
        builder.build();
    }
}
