mod disjoint_sets;
mod resolver;

use std::{
    cell::RefCell,
    collections::{hash_map, HashMap},
    rc::Rc,
};

use crate::{
    pure_values::{Float, Integer},
    util::imm_string::ImmString,
};

use self::{
    disjoint_sets::{DisjointSet, SetIndex},
    resolver::ValueResolver,
};

use super::{
    const_table::{ConstFunction, ConstIndex, ConstValue},
    error::{BuilderError, Result},
    instructions::{CallInstruction, CompareOp, InstructionListBuilder, StackIndex},
    modules::{ConstModule, ImportSource, ModuleId, ModuleMemberId},
};

// The final index of a value in the module. This can be either one of the const indexes,
// or a global
#[derive(Clone, Debug)]
enum ValueIndex {
    Const(ConstIndex),
    Global(u32),
}
impl ValueIndex {
    pub fn as_module_const(&self) -> Result<u32> {
        match self {
            ValueIndex::Const(ConstIndex::ModuleConst(index)) => Ok(*index),
            _ => Err(BuilderError::ExpectedModuleConst),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RefIndex(SetIndex);

struct RefResolver {
    index_layer: Rc<RefCell<DisjointSet<ValueIndex>>>,
}

impl RefResolver {
    pub fn resolve_ref(&self, index: RefIndex) -> Result<ValueIndex> {
        self.index_layer
            .borrow()
            .find(index.0)
            .cloned()
            .ok_or(BuilderError::UnresolvedReference)
    }

    pub fn resolve_to_const_index(&self, index: RefIndex) -> Result<ConstIndex> {
        let ValueIndex::Const(const_index) = self.resolve_ref(index)? else {
            return Err(BuilderError::ExpectedNonGlobal);
        };
        Ok(const_index)
    }
}

struct BuilderInner {
    id: ModuleId,
    ref_indexes: Rc<RefCell<DisjointSet<ValueIndex>>>,
    imports: Vec<ImportSource>,
    values: ValueResolver<RefResolver, ConstValue, BuilderError>,
    exports: HashMap<ModuleMemberId, RefIndex>,
    initializer: Option<RefIndex>,
    num_globals: u32,
}

impl BuilderInner {
    pub fn new_global(&mut self) -> RefIndex {
        let global_index = self.num_globals;
        self.num_globals += 1;
        self.new_ref(ValueIndex::Global(global_index))
    }

    pub fn new_import(&mut self, source: ImportSource) -> RefIndex {
        let import_index = u32::try_from(self.imports.len()).unwrap();
        self.imports.push(source);
        self.new_ref(ValueIndex::Const(ConstIndex::ModuleImport(import_index)))
    }

    pub fn new_const<F>(&mut self, value_fn: F) -> RefIndex
    where
        F: FnOnce(&RefResolver) -> Result<ConstValue> + 'static,
    {
        let resolve_ref = self.values.resolve_ref(value_fn) as u32;
        self.new_ref(ValueIndex::Const(ConstIndex::ModuleConst(resolve_ref)))
    }

    pub fn new_ref(&mut self, value: ValueIndex) -> RefIndex {
        let index = self.ref_indexes.borrow_mut().make_deferred_set();
        self.ref_indexes
            .borrow_mut()
            .resolve_set(index, value)
            .expect("Index should be valid.");
        RefIndex(index)
    }

    pub fn new_deferred_ref(&mut self) -> RefIndex {
        RefIndex(self.ref_indexes.borrow_mut().make_deferred_set())
    }

    pub fn resolve_const<F>(&mut self, index: RefIndex, value_fn: F) -> Result<()>
    where
        F: FnOnce(&RefResolver) -> Result<ConstValue> + 'static,
    {
        let resolve_ref = self.values.resolve_ref(value_fn) as u32;
        self.resolve_ref(
            index,
            ValueIndex::Const(ConstIndex::ModuleConst(resolve_ref)),
        );
        Ok(())
    }

    pub fn resolve_ref(&mut self, index: RefIndex, value: ValueIndex) -> Result<()> {
        Ok(self
            .ref_indexes
            .borrow_mut()
            .resolve_set(index.0, value)
            .expect("Index should be valid."))
    }
}

#[derive(Clone)]
struct InnerRc(Rc<RefCell<BuilderInner>>);

impl InnerRc {
    pub fn new(id: ModuleId) -> Self {
        InnerRc(Rc::new(RefCell::new(BuilderInner {
            id,
            imports: Vec::new(),
            ref_indexes: Rc::new(RefCell::new(DisjointSet::new())),
            values: ValueResolver::new(),
            exports: HashMap::new(),
            initializer: None,
            num_globals: 0,
        })))
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0.as_ptr(), other.0.as_ptr())
    }

    pub fn add_import(&self, source: ImportSource) -> ValueRef {
        let mut inner = self.0.borrow_mut();
        ValueRef {
            builder_inner: self.clone(),
            const_index: inner.new_import(source),
        }
    }

    fn new_ref_with_resolver<F>(&self, resolver: F) -> ValueRef
    where
        F: FnOnce(&RefResolver) -> Result<ConstValue> + 'static,
    {
        let mut inner = self.0.borrow_mut();
        ValueRef {
            builder_inner: self.clone(),
            const_index: inner.new_const(resolver),
        }
    }

    fn new_const_cell(&self, value: ConstValue) -> ValueRef {
        self.new_ref_with_resolver(|_| Ok(value))
    }

    pub fn new_deferred(&self) -> (ValueRef, DeferredValue) {
        let mut inner = self.0.borrow_mut();
        let value_ref = ValueRef {
            builder_inner: self.clone(),
            const_index: inner.new_deferred_ref(),
        };
        let deferred_value = DeferredValue(value_ref.clone());
        (value_ref, deferred_value)
    }

    pub fn new_int(&self, int_value: impl Into<Integer>) -> ValueRef {
        self.new_const_cell(ConstValue::Integer(int_value.into()))
    }

    pub fn new_float(&self, float_value: impl Into<Float>) -> ValueRef {
        self.new_const_cell(ConstValue::Float(float_value.into()))
    }

    pub fn new_bool(&self, bool_value: bool) -> ValueRef {
        self.new_const_cell(ConstValue::Bool(bool_value))
    }

    pub fn new_list(&self, iter: impl IntoIterator<Item = ValueRef>) -> ValueRef {
        let indexes = iter.into_iter().map(|v| v.const_index).collect::<Vec<_>>();
        self.new_ref_with_resolver(move |resolver| {
            Ok(ConstValue::List(
                indexes
                    .into_iter()
                    .map(|v| resolver.resolve_to_const_index(v))
                    .collect::<Result<Vec<_>>>()?,
            ))
        })
    }

    pub fn new_function(&self) -> (ValueRef, FunctionBuilder) {
        let (value_ref, deferred) = self.new_deferred();
        let builder = FunctionBuilder {
            builder_inner: self.clone(),
            deferred,
            const_indexes: Vec::new(),
            insts: InstructionListBuilder::new(),
        };

        (value_ref, builder)
    }

    pub fn new_initializer(&self) -> Result<FunctionBuilder> {
        let (value_ref, deferred) = self.new_deferred();
        let mut inner = self.0.borrow_mut();
        if inner.initializer.is_some() {
            return Err(BuilderError::AlreadyExists);
        }
        inner.initializer = Some(value_ref.const_index);

        Ok(FunctionBuilder {
            builder_inner: self.clone(),
            deferred,
            const_indexes: Vec::new(),
            insts: InstructionListBuilder::new(),
        })
    }

    fn find_ref_index(&self, value_ref: &ValueRef) -> Result<RefIndex> {
        if !self.ptr_eq(&value_ref.builder_inner) {
            return Err(BuilderError::MismatchedBuilder);
        }
        Ok(value_ref.const_index.clone())
    }

    pub fn to_const_module(&self) -> Result<ConstModule> {
        let mut inner = self.0.borrow_mut();
        let exports = inner
            .exports
            .iter()
            .map(|(k, v)| {
                Ok((
                    k.clone(),
                    inner
                        .ref_indexes
                        .borrow()
                        .find(v.0)
                        .ok_or(BuilderError::UnresolvedReference)?
                        .as_module_const()
                        .expect("Expected module const."),
                ))
            })
            .collect::<Result<HashMap<_, _>>>()?;
        let initializer_index = inner
            .initializer
            .as_ref()
            .map(|i| {
                Ok::<_, BuilderError>(
                    inner
                        .ref_indexes
                        .borrow()
                        .find(i.0)
                        .ok_or(BuilderError::UnresolvedReference)?
                        .as_module_const()
                        .expect("Expected module const."),
                )
            })
            .transpose()?;
        let result = std::mem::take(&mut inner.values)
            .into_values(&RefResolver {
                index_layer: inner.ref_indexes.clone(),
            })
            .map_err(BuilderError::new_other)?;
        Ok(ConstModule::new(
            inner.id.clone(),
            result,
            inner.imports.clone(),
            exports,
            initializer_index,
            inner.num_globals,
        )?)
    }
}

pub struct ModuleBuilder(InnerRc);

impl ModuleBuilder {
    pub fn new(id: ModuleId) -> Self {
        ModuleBuilder(InnerRc::new(id))
    }

    pub fn add_import(&self, source: ImportSource) -> ValueRef {
        self.0.add_import(source)
    }

    pub fn new_global(&self) -> GlobalValueRef {
        let index = {
            let mut inner = self.0 .0.borrow_mut();
            let index = inner.num_globals;
            inner.num_globals += 1;
            index
        };
        GlobalValueRef {
            builder_inner: self.0.clone(),
            index,
        }
    }

    pub fn new_deferred(&self) -> (ValueRef, DeferredValue) {
        self.0.new_deferred()
    }

    pub fn new_int(&self, int_value: impl Into<Integer>) -> ValueRef {
        self.0.new_int(int_value)
    }

    pub fn new_float(&self, float_value: impl Into<Float>) -> ValueRef {
        self.0.new_float(float_value)
    }

    pub fn new_bool(&self, bool_value: bool) -> ValueRef {
        self.0.new_bool(bool_value)
    }

    pub fn new_list(&self, iter: impl IntoIterator<Item = ValueRef>) -> ValueRef {
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

#[derive(Clone, Debug)]
struct ValueRefIndex(RefIndex);

impl ValueRefIndex {
    pub fn as_module_const(&self) -> RefIndex {
        self.0
    }

    fn resolve_to_const_index(&self, resolver: &RefResolver) -> Result<ValueIndex> {
        resolver.resolve_ref(self.0)
    }
}

#[derive(Clone)]
pub struct ValueRef {
    builder_inner: InnerRc,
    const_index: RefIndex,
}

impl ValueRef {
    fn resolve_fn<F>(&self, resolve_fn: F) -> Result<()>
    where
        F: FnOnce(&RefResolver) -> Result<ConstValue> + 'static,
    {
        let mut inner = self.builder_inner.0.borrow_mut();
        inner.resolve_const(self.const_index, resolve_fn)?;
        Ok(())
    }

    fn resolve_other(&self, other: &ValueRef) -> Result<()> {
        assert!(Rc::ptr_eq(&self.builder_inner.0, &other.builder_inner.0));
        let inner = self.builder_inner.0.borrow_mut();
        inner
            .ref_indexes
            .borrow_mut()
            .resolve_to_other_set(self.const_index.0, other.const_index.0)
            .map_err(BuilderError::new_other)?;
        Ok(())
    }

    pub fn export(&self, name: ModuleMemberId) -> Result<()> {
        let mut inner = self.builder_inner.0.borrow_mut();
        match inner.exports.entry(name) {
            hash_map::Entry::Occupied(_) => {
                return Err(BuilderError::AlreadyExists);
            }
            hash_map::Entry::Vacant(vac) => {
                vac.insert(self.const_index);
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
    fn resolve_fn<F>(&self, resolve_fn: F) -> Result<()>
    where
        F: FnOnce(&RefResolver) -> Result<ConstValue> + 'static,
    {
        self.0.resolve_fn(resolve_fn)
    }

    fn resolve(self, value: ConstValue) -> Result<()> {
        self.resolve_fn(move |_| Ok(value))
    }

    fn find_ref_index(&self, value_ref: &ValueRef) -> Result<RefIndex> {
        self.0.builder_inner.find_ref_index(value_ref)
    }

    pub fn resolve_int(self, value: impl Into<Integer>) -> Result<()> {
        self.resolve(ConstValue::Integer(value.into()))
    }

    pub fn resolve_float(self, value: impl Into<Float>) -> Result<()> {
        self.resolve(ConstValue::Float(value.into()))
    }

    pub fn resolve_bool(self, value: bool) -> Result<()> {
        self.resolve(ConstValue::Bool(value))
    }

    pub fn resolve_string(self, value: impl Into<ImmString>) -> Result<()> {
        self.resolve(ConstValue::String(value.into()))
    }

    pub fn resolve_list(self, iter: impl IntoIterator<Item = ValueRef>) -> Result<()> {
        let values = iter
            .into_iter()
            .map(|v| self.find_ref_index(&v))
            .collect::<Result<Vec<_>>>()?;
        self.resolve_fn(|resolver| {
            Ok(ConstValue::List(
                values
                    .into_iter()
                    .map(|v| resolver.resolve_to_const_index(v))
                    .collect::<Result<Vec<_>>>()?,
            ))
        })
    }

    pub fn resolve_other(self, value: &ValueRef) -> Result<()> {
        self.0.resolve_other(value)
    }

    pub fn into_function_builder(self) -> FunctionBuilder {
        FunctionBuilder {
            builder_inner: self.0.builder_inner.clone(),
            deferred: self,
            const_indexes: Vec::new(),
            insts: InstructionListBuilder::new(),
        }
    }
}

#[derive(Clone)]
pub struct GlobalValueRef {
    builder_inner: InnerRc,
    index: u32,
}

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
    pub fn push_int(&mut self, value: impl Into<Integer>) -> &mut Self {
        let value_ref = self.builder_inner.new_int(value);
        self.push_value(&value_ref)
            .expect("Value should be resolved.")
    }

    pub fn push_value(&mut self, value: &ValueRef) -> Result<&mut Self> {
        let const_index = self.builder_inner.find_ref_index(value)?;
        let function_const_index = self.const_indexes.len();
        self.const_indexes.push(const_index.clone());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_atomic_values() -> anyhow::Result<()> {
        let value_set = ModuleBuilder::new(ModuleId::new(["foo"]));
        value_set.new_int(42);
        value_set.into_const_module()?;
        Ok(())
    }

    #[test]
    fn test_build_compound_value() -> anyhow::Result<()> {
        let value_set = ModuleBuilder::new(ModuleId::new(["foo"]));
        let i1 = value_set.new_int(42);
        let i2 = value_set.new_int(1138);
        let _list = value_set.new_list(vec![i1.clone(), i2.clone()]);
        let _const_table = value_set.into_const_module()?;
        Ok(())
    }

    #[test]
    fn test_build_function() -> anyhow::Result<()> {
        let value_set = ModuleBuilder::new(ModuleId::new(["foo"]));
        let (f, mut builder) = value_set.new_function();
        builder.push_int(42).push_int(1138).add().return_(1);
        builder.build()?;
        f.export(ModuleMemberId::new("test"))?;
        let _const_table = value_set.into_const_module()?;
        Ok(())
    }
}
