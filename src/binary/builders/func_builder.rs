use crate::{
    binary::{
        error::{BuilderError, Result},
        instructions::{CallInstruction, CompareOp, InstructionListBuilder, StackIndex},
        ConstFunction, ConstValue,
    },
    pure_values::Integer,
};

use super::{DeferredValue, InnerRc, RefIndex, ValueIndex, ValueRef};

pub struct FunctionBuilder {
    builder_inner: InnerRc,
    /// The value reference for the deferred function being built.
    deferred: DeferredValue,
    value_pushes: Vec<(u32, RefIndex)>,
    value_pops: Vec<(u32, RefIndex)>,
    insts: InstructionListBuilder,
}

macro_rules! def_build_inst_method {
    ($method:ident($($arg:ident : $arg_type:ty),*)) => {
        pub fn $method(&mut self, $($arg: $arg_type),*) -> &mut Self {
            self.insts.$method($($arg),*);
            self
        }
    };
}

impl FunctionBuilder {
    pub(super) fn new(builder_inner: InnerRc, deferred: DeferredValue) -> Self {
        FunctionBuilder {
            builder_inner,
            deferred,
            value_pushes: Vec::new(),
            value_pops: Vec::new(),
            insts: InstructionListBuilder::new(),
        }
    }
    pub fn push_int(&mut self, value: impl Into<Integer>) -> &mut Self {
        let value_ref = self.builder_inner.new_int(value);
        self.push_value(&value_ref)
            .expect("Value should be resolved.")
    }

    pub fn push_value(&mut self, value: &ValueRef) -> Result<&mut Self> {
        let ref_index = self.builder_inner.find_ref_index(value)?;
        let inst_index = self.insts.add_deferred_inst();
        self.value_pushes.push((inst_index, ref_index));
        Ok(self)
    }

    pub fn pop_value(&mut self, value: &ValueRef) -> Result<&mut Self> {
        let ref_index = self.builder_inner.find_ref_index(value)?;
        let inst_index = self.insts.add_deferred_inst();
        self.value_pops.push((inst_index, ref_index));
        Ok(self)
    }

    def_build_inst_method!(add());
    def_build_inst_method!(push_copy(s: StackIndex));
    def_build_inst_method!(pop(n: u32));
    def_build_inst_method!(write_stack(s: StackIndex));
    def_build_inst_method!(bool_and());
    def_build_inst_method!(bool_or());
    def_build_inst_method!(bool_xor());
    def_build_inst_method!(bool_not());
    def_build_inst_method!(compare(op: CompareOp));
    def_build_inst_method!(call(call: CallInstruction));
    def_build_inst_method!(tail_call(num_args: u32));
    def_build_inst_method!(call_dynamic());
    def_build_inst_method!(return_(n: u32));
    def_build_inst_method!(return_dynamic());
    def_build_inst_method!(branch_if(target: &str));
    def_build_inst_method!(branch(target: &str));
    def_build_inst_method!(define_branch_target(target: &str));
    def_build_inst_method!(bind_front(num_args: u32));

    pub fn build(self) -> Result<()> {
        let mut instructions = self.insts;
        let value_pushes = self.value_pushes;
        let value_pops = self.value_pops;

        self.deferred.resolve_fn(|resolver| {
            let mut const_indexes = Vec::new();
            for (inst_index, ref_index) in value_pushes {
                match resolver.resolve_ref(ref_index)? {
                    ValueIndex::Const(const_index) => {
                        let local_index = const_indexes.len();
                        const_indexes.push(const_index);
                        instructions.resolve_push_const(inst_index, local_index as u32)?;
                    }
                    super::ValueIndex::Global(global_index) => {
                        instructions.resolve_push_global(inst_index, global_index)?;
                    }
                }
            }
            for (inst_index, ref_index) in value_pops {
                match resolver.resolve_ref(ref_index)? {
                    ValueIndex::Const(_) => {
                        return Err(BuilderError::ExpectedGlobal);
                    }
                    super::ValueIndex::Global(global_index) => {
                        instructions.resolve_pop_global(inst_index, global_index)?;
                    }
                }
            }
            Ok(ConstValue::Function(ConstFunction::new(
                const_indexes,
                instructions.build()?,
            )))
        })?;
        Ok(())
    }
}
