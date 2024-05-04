use std::{
    cell::RefCell,
    collections::{hash_map, HashMap},
    rc::Rc,
};

use crate::{
    pure_values::{Float, Integer},
    util::imm_string::ImmString,
};

use super::{
    const_table::{ConstFunction, ConstIndex, ConstValue},
    error::{BuilderError, Result},
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

    fn new_const_cell(&self, value: Option<ConstValue>) -> ValueRef {
        let index = {
            let mut inner = self.0.borrow_mut();
            let index = inner.values.len();
            inner.values.push(value);
            index
        };
        ValueRef {
            builder_inner: self.clone(),
            const_index: ConstIndex::ModuleConst(u32::try_from(index).unwrap()),
        }
    }

    pub fn new_deferred(&self) -> (ValueRef, DeferredValue) {
        let value_ref = self.new_const_cell(None);
        let deferred_value = DeferredValue(value_ref.clone());
        (value_ref, deferred_value)
    }

    pub fn new_int(&self, int_value: impl Into<Integer>) -> ValueRef {
        self.new_const_cell(Some(ConstValue::Integer(int_value.into())))
    }

    pub fn new_list(&self, iter: impl IntoIterator<Item = ValueRef>) -> Result<ValueRef> {
        let list_const = ConstValue::List(
            iter.into_iter()
                .map(|v| self.find_ref_index(&v))
                .collect::<Result<Vec<_>>>()?,
        );
        Ok(self.new_const_cell(Some(list_const)))
    }

    pub fn new_function(&self) -> (ValueRef, FunctionBuilder) {
        let value_ref = self.new_const_cell(None);
        let builder = FunctionBuilder {
            value_ref: value_ref.clone(),
            const_indexes: Vec::new(),
            insts: InstructionListBuilder::new(),
        };

        (value_ref, builder)
    }

    pub fn new_initializer(&self) -> Result<FunctionBuilder> {
        let value_ref = self.new_const_cell(None);
        let index = value_ref
            .const_index
            .as_module_const()
            .expect("Expected module const.");
        let mut inner = self.0.borrow_mut();
        if inner.initializer.is_some() {
            return Err(BuilderError::AlreadyExists);
        }
        inner.initializer = Some(index);

        Ok(FunctionBuilder {
            value_ref,
            const_indexes: Vec::new(),
            insts: InstructionListBuilder::new(),
        })
    }

    fn find_ref_index(&self, value_ref: &ValueRef) -> Result<ConstIndex> {
        if !self.ptr_eq(&value_ref.builder_inner) {
            return Err(BuilderError::MismatchedBuilder);
        }
        Ok(value_ref.const_index.clone())
    }

    pub fn to_const_module(&self) -> Result<ConstModule> {
        let mut result = Vec::new();
        let inner = self.0.borrow();
        for value in inner.values.iter() {
            result.push(
                value
                    .as_ref()
                    .ok_or(BuilderError::DeferredNotResolved)?
                    .clone(),
            );
        }
        Ok(ConstModule::new(
            result,
            inner.imports.clone(),
            inner.exports.clone(),
            inner.initializer,
            inner.num_globals,
        )?)
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

    pub fn new_list(&self, iter: impl IntoIterator<Item = ValueRef>) -> Result<ValueRef> {
        self.0.new_list(iter)
    }

    pub fn new_function(&self) -> (ValueRef, FunctionBuilder) {
        self.0.new_function()
    }

    pub fn new_initializer(&self) -> Result<FunctionBuilder> {
        self.0.new_initializer()
    }

    pub fn into_const_module(&self) -> Result<ConstModule> {
        self.0.to_const_module()
    }
}

#[derive(Clone)]
pub struct ValueRef {
    builder_inner: InnerRc,
    const_index: ConstIndex,
}

impl ValueRef {
    fn resolve(&self, const_value: ConstValue) -> Result<()> {
        let mut inner = self.builder_inner.0.borrow_mut();
        let index = self
            .const_index
            .as_module_const()
            .expect("Only module consts can be resolved.");
        let cell = &mut inner.values[index as usize];
        if cell.is_some() {
            return Err(BuilderError::AlreadyExists);
        }
        *cell = Some(const_value);
        Ok(())
    }

    pub fn export(&self, name: ModuleMemberId) -> Result<()> {
        let mut inner = self.builder_inner.0.borrow_mut();
        let index = self
            .const_index
            .as_module_const()
            .ok_or(BuilderError::ExpectedModuleConst)?;
        match inner.exports.entry(name) {
            hash_map::Entry::Occupied(_) => {
                return Err(BuilderError::AlreadyExists);
            }
            hash_map::Entry::Vacant(vac) => {
                vac.insert(index);
            }
        }
        Ok(())
    }
}

/// Represents a value that still needs to be resolved.
///
/// Dropping this value without resolving it will panic.
pub struct DeferredValue(ValueRef);

impl DeferredValue {
    fn resolve(&self, const_value: ConstValue) -> Result<()> {
        self.0.resolve(const_value)
    }

    fn find_ref_index(&self, value_ref: &ValueRef) -> Result<ConstIndex> {
        self.0.builder_inner.find_ref_index(value_ref)
    }

    pub fn resolve_int(self, value: impl Into<Integer>) -> Result<()> {
        self.resolve(ConstValue::Integer(value.into()))
    }

    pub fn resolve_float(self, value: impl Into<Float>) -> Result<()> {
        self.resolve(ConstValue::Float(value.into()))
    }

    pub fn resolve_string(self, value: impl Into<ImmString>) -> Result<()> {
        self.resolve(ConstValue::String(value.into()))
    }

    pub fn resolve_list(self, iter: impl IntoIterator<Item = ValueRef>) -> Result<()> {
        let values = iter
            .into_iter()
            .map(|v| self.find_ref_index(&v))
            .collect::<Result<Vec<_>>>()?;
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

impl Drop for DeferredValue {
    fn drop(&mut self) {
        match self.0.const_index {
            ConstIndex::ModuleConst(index) => {
                let inner = self.0.builder_inner.0.borrow();
                if inner.values[index as usize].is_none() {
                    panic!("Deferred value not resolved.");
                }
            }
            _ => panic!("Invalid const index."),
        }
    }
}

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
        self.push_value(&value_ref)
            .expect("Value should be resolved.")
    }

    pub fn push_value(&mut self, value: &ValueRef) -> Result<&mut Self> {
        let const_index = self.value_ref.builder_inner.find_ref_index(value)?;
        let function_const_index = self.const_indexes.len();
        self.const_indexes.push(const_index.clone());
        self.insts.push_const(function_const_index as u32);
        Ok(self)
    }

    def_build_inst_method!(add());
    def_build_inst_method!(push_copy(s: StackIndex));
    def_build_inst_method!(push_global(index: u32));
    def_build_inst_method!(pop_global(index: u32));
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

    pub fn build(mut self) -> Result<()> {
        self.value_ref
            .resolve(ConstValue::Function(ConstFunction::new(
                std::mem::take(&mut self.const_indexes),
                std::mem::take(&mut self.insts).build()?,
            )))?;
        Ok(())
    }
}

impl Drop for FunctionBuilder {
    fn drop(&mut self) {
        match self.value_ref.const_index {
            ConstIndex::ModuleConst(index) => {
                let inner = self.value_ref.builder_inner.0.borrow();
                if inner.values[index as usize].is_none() {
                    panic!("Deferred value not resolved.");
                }
            }
            _ => panic!("Invalid const index."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_atomic_values() -> anyhow::Result<()> {
        let value_set = ModuleBuilder::with_num_globals(0);
        value_set.new_int(42);
        value_set.into_const_module()?;
        Ok(())
    }

    #[test]
    fn test_build_compound_value() -> anyhow::Result<()> {
        let value_set = ModuleBuilder::with_num_globals(0);
        let i1 = value_set.new_int(42);
        let i2 = value_set.new_int(1138);
        let _list = value_set.new_list(vec![i1.clone(), i2.clone()]);
        let _const_table = value_set.into_const_module()?;
        Ok(())
    }

    #[test]
    fn test_build_function() -> anyhow::Result<()> {
        let value_set = ModuleBuilder::with_num_globals(0);
        let (f, mut builder) = value_set.new_function();
        builder.push_int(42).push_int(1138).add().return_(1);
        builder.build()?;
        f.export(ModuleMemberId::new("test"))?;
        let _const_table = value_set.into_const_module()?;
        Ok(())
    }
}
