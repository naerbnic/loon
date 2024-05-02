use std::{cell::RefCell, rc::Rc};

use crate::{
    pure_values::{Float, Integer},
    util::imm_string::ImmString,
};

use super::{
    const_table::{ConstFunction, ConstIndex, ConstTable, ConstValue, LayerIndex},
    instructions::{CallInstruction, CompareOp, InstructionListBuilder, StackIndex},
};

struct ValueSetInner {
    parent: Option<InnerRc>,
    values: Vec<Option<ConstValue>>,
}

#[derive(Clone)]
struct InnerRc(Rc<RefCell<ValueSetInner>>);

impl InnerRc {
    pub fn new(parent: Option<InnerRc>, values: Vec<Option<ConstValue>>) -> Self {
        InnerRc(Rc::new(RefCell::new(ValueSetInner { parent, values })))
    }

    pub fn new_child(&self) -> Self {
        InnerRc::new(Some(self.clone()), Vec::new())
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0.as_ptr(), other.0.as_ptr())
    }

    fn new_external(&self, value: ConstIndex) -> ValueRef {
        let index = {
            let mut inner = self.0.borrow_mut();
            let index = inner.values.len();
            inner.values.push(Some(ConstValue::ExternalRef(value)));
            index
        };
        ValueRef {
            value_set: self.clone(),
            index,
        }
    }

    pub fn new_deferred(&self) -> (ValueRef, DeferredValue) {
        let index = {
            let mut inner = self.0.borrow_mut();
            let index = inner.values.len();
            inner.values.push(None);
            index
        };
        let value_ref = ValueRef {
            value_set: self.clone(),
            index,
        };
        let deferred_value = DeferredValue(value_ref.clone());
        (value_ref, deferred_value)
    }

    pub fn new_int(&self, int_value: impl Into<Integer>) -> ValueRef {
        let (value, def) = self.new_deferred();
        def.resolve_int(int_value);
        value
    }

    pub fn new_list(&self, iter: impl IntoIterator<Item = ValueRef>) -> ValueRef {
        let (value, def) = self.new_deferred();
        def.resolve_list(iter);
        value
    }

    pub fn new_function(&self) -> (ValueRef, FunctionBuilder) {
        let (value_ref, deferred_value) = self.new_deferred();
        let builder = deferred_value.into_function_builder();
        (value_ref, builder)
    }

    fn find_ref_index(&self, value_ref: &ValueRef) -> Option<ConstIndex> {
        let mut curr_layer = 0;
        let mut curr_set = self.clone();
        while !curr_set.ptr_eq(&value_ref.value_set) {
            let next_set = {
                let inner = curr_set.0.borrow();
                curr_layer += 1;
                inner.parent.as_ref()?.clone()
            };
            curr_set = next_set;
        }
        Some(ConstIndex::Local(LayerIndex::new(
            curr_layer,
            value_ref.index,
        )))
    }

    pub fn to_const_table(&self) -> ConstTable {
        let mut result = Vec::new();
        for value in self.0.borrow().values.iter() {
            if let Some(value) = value {
                result.push(value.clone());
            } else {
                panic!("Deferred value not resolved.");
            }
        }
        ConstTable::new(result).expect("Failed to create const table.")
    }
}

pub struct ValueSet(InnerRc);

impl ValueSet {
    pub fn new_root() -> Self {
        ValueSet(InnerRc::new(None, Vec::new()))
    }

    pub fn new_child(parent: &ValueSet) -> Self {
        ValueSet(InnerRc::new(Some(parent.0.clone()), Vec::new()))
    }

    pub fn new_deferred(&self) -> (ValueRef, DeferredValue) {
        self.0.new_deferred()
    }

    pub fn new_int(&self, int_value: impl Into<Integer>) -> ValueRef {
        self.0.new_int(int_value)
    }

    pub fn new_list(&self, iter: impl IntoIterator<Item = ValueRef>) -> ValueRef {
        self.0.new_list(iter)
    }

    pub fn new_function(&self) -> (ValueRef, FunctionBuilder) {
        self.0.new_function()
    }

    pub fn into_const_table(&self) -> ConstTable {
        self.0.to_const_table()
    }
}

#[derive(Clone)]
pub struct ValueRef {
    value_set: InnerRc,
    index: usize,
}

impl ValueRef {
    fn resolve(&self, const_value: ConstValue) {
        let mut inner = self.value_set.0.borrow_mut();
        let prev = inner.values[self.index].replace(const_value);
        assert!(prev.is_none());
    }
}

/// Represents a value that still needs to be resolved.
///
/// Dropping this value without resolving it will panic.
pub struct DeferredValue(ValueRef);

impl DeferredValue {
    fn resolve(&self, const_value: ConstValue) {
        self.0.resolve(const_value);
    }

    fn find_ref_index(&self, value_ref: &ValueRef) -> Option<ConstIndex> {
        self.0.value_set.find_ref_index(value_ref)
    }

    pub fn resolve_int(self, value: impl Into<Integer>) {
        self.resolve(ConstValue::Integer(value.into()))
    }

    pub fn resolve_float(self, value: impl Into<Float>) {
        self.resolve(ConstValue::Float(value.into()))
    }

    pub fn resolve_string(self, value: impl Into<ImmString>) {
        self.resolve(ConstValue::String(value.into()))
    }

    pub fn resolve_list(self, iter: impl IntoIterator<Item = ValueRef>) {
        let values = iter
            .into_iter()
            .map(|v| self.find_ref_index(&v).expect("Invalid reference"))
            .collect();
        self.resolve(ConstValue::List(values))
    }

    pub fn into_function_builder(self) -> FunctionBuilder {
        FunctionBuilder {
            value_ref: self.0.clone(),
            local_value_set: self.0.value_set.new_child(),
            insts: InstructionListBuilder::new(),
        }
    }
}

impl Drop for DeferredValue {
    fn drop(&mut self) {
        self.0.value_set.0.borrow().values[self.0.index]
            .as_ref()
            .expect("Deferred value not resolved.");
    }
}

pub struct BranchTarget();

pub struct ModuleBuilder {}

impl ModuleBuilder {}

pub struct FunctionBuilder {
    /// The value reference for the deferred function being built.
    value_ref: ValueRef,
    local_value_set: InnerRc,
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
    pub fn push_int(&mut self, value: impl Into<Integer>) -> &mut Self {
        let value_ref = self.local_value_set.new_int(value);
        self.push_value(&value_ref);
        self
    }

    pub fn push_value(&mut self, value: &ValueRef) -> &mut Self {
        let const_index = self.local_value_set.find_ref_index(value).unwrap();
        let index = if let Some(parent_index) = const_index.in_prev_layer() {
            let value_ref = self.local_value_set.new_external(parent_index);
            value_ref.index
        } else if let ConstIndex::Local(index) = const_index {
            index.index()
        } else {
            panic!("Invalid const index.");
        };
        self.insts.push_const(index as u32);
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
    def_build_inst_method!(define_branch_target(target: &str));

    pub fn build(self) {
        self.value_ref
            .resolve(ConstValue::Function(ConstFunction::new(
                self.local_value_set.to_const_table(),
                self.insts.build(),
            )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_atomic_values() {
        let value_set = ValueSet::new_root();
        value_set.new_int(42);
        value_set.into_const_table();
    }

    #[test]
    fn test_build_compound_value() {
        let value_set = ValueSet::new_root();
        let i1 = value_set.new_int(42);
        let i2 = value_set.new_int(1138);
        let list = value_set.new_list(vec![i1.clone(), i2.clone()]);
        let const_table = value_set.into_const_table();
        assert!(!const_table.constraints().needs_parent_layers());
    }
}
