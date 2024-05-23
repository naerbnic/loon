use crate::{
    binary::{
        error::Result,
        instructions::{CallInstruction, CompareOp, InstructionListBuilder, StackIndex},
        ConstFunction, ConstValue,
    },
    pure_values::Integer,
};

use super::{DeferredValue, GlobalValueRef, InnerRc, RefIndex, ValueRef};

pub struct FunctionBuilder {
    builder_inner: InnerRc,
    /// The value reference for the deferred function being built.
    deferred: DeferredValue,
    const_indexes: Vec<RefIndex>,
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
            const_indexes: Vec::new(),
            insts: InstructionListBuilder::new(),
        }
    }
    pub fn push_int(&mut self, value: impl Into<Integer>) -> &mut Self {
        let value_ref = self.builder_inner.new_int(value);
        self.push_value(&value_ref)
            .expect("Value should be resolved.")
    }

    pub fn push_value(&mut self, value: &ValueRef) -> Result<&mut Self> {
        let const_index = self.builder_inner.find_ref_index(value)?;
        let function_const_index = self.const_indexes.len();
        self.const_indexes.push(const_index);
        self.insts.push_const(function_const_index as u32);
        Ok(self)
    }

    pub fn push_global(&mut self, value: &GlobalValueRef) -> &mut Self {
        assert!(self.builder_inner.ptr_eq(&value.builder_inner));
        self.insts.push_global(value.index);
        self
    }

    pub fn pop_global(&mut self, value: &GlobalValueRef) -> &mut Self {
        assert!(self.builder_inner.ptr_eq(&value.builder_inner));
        self.insts.pop_global(value.index);
        self
    }

    def_build_inst_method!(add());
    def_build_inst_method!(push_copy(s: StackIndex));
    def_build_inst_method!(pop(n: u32));
    def_build_inst_method!(bool_and());
    def_build_inst_method!(bool_or());
    def_build_inst_method!(bool_xor());
    def_build_inst_method!(bool_not());
    def_build_inst_method!(compare(op: CompareOp));
    def_build_inst_method!(call(call: CallInstruction));
    def_build_inst_method!(call_dynamic());
    def_build_inst_method!(return_(n: u32));
    def_build_inst_method!(return_dynamic());
    def_build_inst_method!(branch_if(target: &str));
    def_build_inst_method!(branch(target: &str));
    def_build_inst_method!(define_branch_target(target: &str));

    pub fn build(mut self) -> Result<()> {
        let instructions = std::mem::take(&mut self.insts).build()?;
        let const_indexes = std::mem::take(&mut self.const_indexes);
        self.deferred.resolve_fn(|resolver| {
            Ok(ConstValue::Function(ConstFunction::new(
                const_indexes
                    .into_iter()
                    .map(|i| resolver.resolve_to_const_index(i))
                    .collect::<Result<Vec<_>>>()?,
                instructions,
            )))
        })?;
        Ok(())
    }
}
