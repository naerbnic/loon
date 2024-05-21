use std::{cell::RefCell, collections::HashMap};

use super::{
    error::{Result, RuntimeError},
    inst_set::{
        Add, BoolAnd, BoolNot, BoolOr, BoolXor, Branch, BranchIf, Call, CallDynamic, Compare,
        ListAppend, ListGet, ListLen, ListNew, ListSet, Pop, PushConst, PushCopy, PushGlobal,
        Return, ReturnDynamic, SetGlobal, TailCall,
    },
    instructions::{InstEvalList, InstPtr},
    modules::Module,
    value::{Function, Value},
};
use crate::{
    binary::{
        self,
        instructions::{Instruction, InstructionList},
        modules::{ImportSource, ModuleId},
    },
    gc::{CollectGuard, GcEnv, GcRef, GcRefVisitor, GcTraceable, PinnedGcRef},
};

struct Inner {
    loaded_modules: RefCell<HashMap<ModuleId, Module>>,
}

impl Inner {
    pub fn get_import(&self, import_source: &ImportSource) -> Result<Value> {
        let loaded_modules = self.loaded_modules.borrow();
        loaded_modules
            .get(import_source.module_id())
            .ok_or_else(|| RuntimeError::new_internal_error("Module not found in global context."))?
            .get_export(import_source.import_name())
    }

    pub fn resolve_instructions(&self, inst_list: &InstructionList) -> Result<InstEvalList> {
        let inst_slice = inst_list.instructions();
        let result = inst_slice
            .iter()
            .map(|inst| {
                Ok(match inst {
                    Instruction::PushConst(i) => InstPtr::new(PushConst::new(*i)),
                    Instruction::PushCopy(i) => InstPtr::new(PushCopy::new(*i)),
                    Instruction::PushGlobal(i) => InstPtr::new(PushGlobal::new(*i)),
                    Instruction::PopGlobal(i) => InstPtr::new(SetGlobal::new(*i)),
                    Instruction::Pop(i) => InstPtr::new(Pop::new(*i)),
                    Instruction::Add => InstPtr::new(Add),
                    Instruction::BoolAnd => InstPtr::new(BoolAnd),
                    Instruction::BoolOr => InstPtr::new(BoolOr),
                    Instruction::BoolXor => InstPtr::new(BoolXor),
                    Instruction::BoolNot => InstPtr::new(BoolNot),
                    Instruction::ListNew => InstPtr::new(ListNew),
                    Instruction::ListAppend => InstPtr::new(ListAppend),
                    Instruction::ListLen => InstPtr::new(ListLen),
                    Instruction::ListGet => InstPtr::new(ListGet),
                    Instruction::ListSet => InstPtr::new(ListSet),
                    Instruction::Compare(cmp_op) => InstPtr::new(Compare::new(*cmp_op)),
                    Instruction::Branch(target) => InstPtr::new(Branch::new(*target)),
                    Instruction::BranchIf(target) => InstPtr::new(BranchIf::new(*target)),
                    Instruction::Call(i) => InstPtr::new(Call::new(*i)),
                    Instruction::CallDynamic => InstPtr::new(CallDynamic),
                    Instruction::Return(i) => InstPtr::new(Return::new(*i)),
                    Instruction::ReturnDynamic => InstPtr::new(ReturnDynamic),
                    Instruction::TailCall(i) => InstPtr::new(TailCall::new(*i)),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(InstEvalList::from_inst_ptrs(result))
    }
}

impl GcTraceable for Inner {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        let loaded_modules = self.loaded_modules.borrow();
        for module in loaded_modules.values() {
            module.trace(visitor);
        }
    }
}

#[derive(Clone)]
pub(crate) struct GlobalEnv {
    gc_env: GcEnv,
    inner: PinnedGcRef<Inner>,
}

impl GlobalEnv {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let gc_env = GcEnv::new(1);
        let inner = gc_env.create_pinned_ref(Inner {
            loaded_modules: RefCell::new(HashMap::new()),
        });
        GlobalEnv { gc_env, inner }
    }

    pub fn lock_collect(&self) -> GlobalEnvLock {
        GlobalEnvLock {
            gc_guard: self.gc_env.lock_collect(),
            inner: &self.inner,
        }
    }

    pub fn create_pinned_ref<T>(&self, value: T) -> PinnedGcRef<T>
    where
        T: GcTraceable + 'static,
    {
        self.gc_env.create_pinned_ref(value)
    }

    /// Loads a module into this global context.
    ///
    /// This does not initialize the module state, and has to be done at a
    /// later pass.
    pub fn load_module(&self, const_module: &binary::modules::ConstModule) -> Result<()> {
        let collect_guard = self.lock_collect();
        let module = Module::from_binary(&collect_guard, const_module)?;
        self.inner
            .loaded_modules
            .borrow_mut()
            .insert(const_module.id().clone(), module);
        Ok(())
    }

    pub(super) fn get_init_function(
        &self,
        module_id: &ModuleId,
    ) -> Result<Option<GcRef<Function>>> {
        let loaded_modules = self.inner.loaded_modules.borrow();
        loaded_modules
            .get(module_id)
            .ok_or_else(|| RuntimeError::new_internal_error("Module not found in global context."))?
            .get_init_function()
    }

    pub(super) fn set_module_initialized(&self, module_id: &ModuleId) -> Result<()> {
        let loaded_modules = self.inner.loaded_modules.borrow();
        loaded_modules
            .get(module_id)
            .ok_or_else(|| RuntimeError::new_internal_error("Module not found in global context."))?
            .set_is_initialized();
        Ok(())
    }

    pub(super) fn is_module_loaded(&self, module_id: &ModuleId) -> bool {
        self.inner.loaded_modules.borrow().contains_key(module_id)
    }
}

#[derive(Clone)]
pub(crate) struct GlobalEnvLock<'a> {
    gc_guard: CollectGuard<'a>,
    inner: &'a Inner,
}

impl<'a> GlobalEnvLock<'a> {
    pub fn create_ref<T>(&self, value: T) -> GcRef<T>
    where
        T: GcTraceable + 'static,
    {
        self.gc_guard.create_ref(value)
    }

    pub fn get_import(&self, import_source: &ImportSource) -> Result<Value> {
        self.inner.get_import(import_source)
    }

    pub fn resolve_instructions(&self, inst_list: &InstructionList) -> Result<InstEvalList> {
        self.inner.resolve_instructions(inst_list)
    }
}
