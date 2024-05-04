use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    pure_values::{Float, Integer},
    util::imm_string::ImmString,
};

use super::{
    const_table::{ConstFunction, ConstIndex, ConstTable, ConstValue},
    instructions::{CallInstruction, CompareOp, InstructionListBuilder, StackIndex},
    modules::{ConstModule, ImportSource, ModuleMemberId},
};

struct BuilderInner {
    imports: Vec<ImportSource>,
    values: Vec<Option<ConstValue>>,
    exports: HashMap<ModuleMemberId, u32>,
    initializer: Option<u32>,
    num_globals: u32,
}

#[derive(Clone)]
struct InnerRc(Rc<RefCell<BuilderInner>>);

impl InnerRc {
    pub fn with_num_globals(num_globals: u32) -> Self {
        InnerRc(Rc::new(RefCell::new(BuilderInner {
            imports: Vec::new(),
            values: Vec::new(),
            exports: HashMap::new(),
            initializer: None,
            num_globals,
        })))
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0.as_ptr(), other.0.as_ptr())
    }

    pub fn add_import(&self, source: ImportSource) -> ValueRef {
        let mut inner = self.0.borrow_mut();
        let index = inner.imports.len();
        inner.imports.push(source);
        ValueRef {
            builder_inner: self.clone(),
            const_index: ConstIndex::ModuleImport(u32::try_from(index).unwrap()),
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
            builder_inner: self.clone(),
            const_index: ConstIndex::ModuleConst(u32::try_from(index).unwrap()),
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

    pub fn new_initializer(&self) -> FunctionBuilder {
        let (value_ref, deferred_value) = self.new_deferred();
        {
            let mut inner = self.0.borrow_mut();
            let index = match &value_ref.const_index {
                ConstIndex::ModuleConst(i) => *i,
                ConstIndex::ModuleImport(_) => panic!("Cannot mark an import as an initializer."),
            };
            let prev_initializer = inner.initializer.replace(index);
            assert!(prev_initializer.is_none(), "Initializer already defined.");
        }
        deferred_value.into_function_builder()
    }

    fn find_ref_index(&self, value_ref: &ValueRef) -> Option<ConstIndex> {
        if !self.ptr_eq(&value_ref.builder_inner) {
            return None;
        }
        Some(value_ref.const_index.clone())
    }

    pub fn to_const_module(&self) -> ConstModule {
        let mut result = Vec::new();
        let inner = self.0.borrow();
        dbg!(inner.values.len());
        for value in inner.values.iter() {
            if let Some(value) = value {
                result.push(value.clone());
            } else {
                panic!("Deferred value not resolved.");
            }
        }
        let const_table = ConstTable::new(result).expect("Failed to create const table.");
        ConstModule::new(
            const_table,
            inner.imports.clone(),
            inner.exports.clone(),
            inner.initializer,
            inner.num_globals,
        )
    }
}

pub struct ModuleBuilder(InnerRc);

impl ModuleBuilder {
    pub fn with_num_globals(num_globals: u32) -> Self {
        ModuleBuilder(InnerRc::with_num_globals(num_globals))
    }

    pub fn add_import(&self, source: ImportSource) -> ValueRef {
        self.0.add_import(source)
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

    pub fn new_initializer(&self) -> FunctionBuilder {
        self.0.new_initializer()
    }

    pub fn into_const_module(&self) -> ConstModule {
        self.0.to_const_module()
    }
}

#[derive(Clone)]
pub struct ValueRef {
    builder_inner: InnerRc,
    const_index: ConstIndex,
}

impl ValueRef {
    fn resolve(&self, const_value: ConstValue) {
        let mut inner = self.builder_inner.0.borrow_mut();
        match &self.const_index {
            ConstIndex::ModuleConst(index) => {
                let prev = inner.values[*index as usize].replace(const_value);
                assert!(prev.is_none());
            }
            _ => panic!("Invalid const index."),
        }
    }

    pub fn export(&self, name: ModuleMemberId) {
        let mut inner = self.builder_inner.0.borrow_mut();
        let index = match &self.const_index {
            ConstIndex::ModuleConst(index) => *index,
            _ => panic!("Invalid const index."),
        };
        let prev = inner.exports.insert(name, index);
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
        self.0.builder_inner.find_ref_index(value_ref)
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
            const_indexes: Vec::new(),
            insts: InstructionListBuilder::new(),
        }
    }
}

// impl Drop for DeferredValue {
//     fn drop(&mut self) {
//         match self.0.const_index {
//             ConstIndex::ModuleConst(index) => {
//                 let inner = self.0.builder_inner.0.borrow();
//                 if inner.values[index as usize].is_none() {
//                     panic!("Deferred value not resolved.");
//                 }
//             }
//             _ => panic!("Invalid const index."),
//         }
//     }
// }

pub struct BranchTarget();

pub struct FunctionBuilder {
    /// The value reference for the deferred function being built.
    value_ref: ValueRef,
    const_indexes: Vec<ConstIndex>,
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
        let value_ref = self.value_ref.builder_inner.new_int(value);
        self.push_value(&value_ref);
        self
    }

    pub fn push_value(&mut self, value: &ValueRef) -> &mut Self {
        let const_index = self.value_ref.builder_inner.find_ref_index(value).unwrap();
        let index = if let ConstIndex::ModuleConst(index) = const_index {
            index
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
                self.const_indexes,
                self.insts.build(),
            )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_atomic_values() {
        let value_set = ModuleBuilder::with_num_globals(0);
        value_set.new_int(42);
        value_set.into_const_module();
    }

    #[test]
    fn test_build_compound_value() {
        let value_set = ModuleBuilder::with_num_globals(0);
        let i1 = value_set.new_int(42);
        let i2 = value_set.new_int(1138);
        let _list = value_set.new_list(vec![i1.clone(), i2.clone()]);
        let _const_table = value_set.into_const_module();
    }

    #[test]
    fn test_build_function() {
        let value_set = ModuleBuilder::with_num_globals(0);
        let (f, mut builder) = value_set.new_function();
        builder.push_int(42);
        builder.push_int(1138);
        builder.add();
        builder.return_(1);
        builder.build();
        f.export(ModuleMemberId::new("test"));
        let _const_table = value_set.into_const_module();
    }
}
