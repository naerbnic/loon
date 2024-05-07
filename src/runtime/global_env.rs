use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

use super::{
    error::{Result, RuntimeError},
    eval_context::EvalContextContents,
    inst_set::{
        Add, BoolAnd, BoolNot, BoolOr, BoolXor, Branch, BranchIf, Call, CallDynamic, Compare,
        ListAppend, ListGet, ListLen, ListNew, ListSet, Pop, PushConst, PushCopy, PushGlobal,
        Return, ReturnDynamic, SetGlobal, TailCall,
    },
    instructions::{InstEvalList, InstPtr},
    modules::Module,
    top_level::TopLevelContents,
    value::{Function, Value},
};
use crate::{
    binary::{
        self,
        instructions::{Instruction, InstructionList},
        modules::{ImportSource, ModuleId},
    },
    gc::{GcEnv, GcRef, GcRefVisitor, GcTraceable},
};

struct Inner {
    gc_context: GcEnv,
    loaded_modules: RefCell<HashMap<ModuleId, Module>>,
    top_level_contents: RefCell<HashMap<*const (), TopLevelContents>>,
    eval_context_contents: RefCell<HashMap<*const (), EvalContextContents>>,
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
        for contents in self.top_level_contents.borrow().values() {
            contents.trace(visitor);
        }
    }
}

#[derive(Clone)]
pub struct GlobalEnv(Rc<Inner>);

impl GlobalEnv {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let inner_rc = Rc::new_cyclic(|inner_weak| Inner {
            gc_context: GcEnv::with_root_gatherer(1, {
                let inner: Weak<Inner> = inner_weak.clone();
                move |gc_roots| {
                    let Some(inner) = inner.upgrade() else {
                        return;
                    };
                    {
                        let loaded_modules = inner.loaded_modules.borrow();
                        for value in loaded_modules.values() {
                            gc_roots.visit(value);
                        }
                    }
                }
            }),
            loaded_modules: RefCell::new(HashMap::new()),
            top_level_contents: RefCell::new(HashMap::new()),
            eval_context_contents: RefCell::new(HashMap::new()),
        });
        GlobalEnv(inner_rc)
    }

    pub fn create_ref<T>(&self, value: T) -> GcRef<T>
    where
        T: GcTraceable + 'static,
    {
        self.0.gc_context.create_ref(value)
    }

    /// Loads a module into this global context.
    ///
    /// This does not initialize the module state, and has to be done at a
    /// later pass.
    pub fn load_module(
        &self,
        module_id: ModuleId,
        module: &binary::modules::ConstModule,
    ) -> Result<()> {
        let module = Module::from_binary(self, module)?;
        self.0.loaded_modules.borrow_mut().insert(module_id, module);
        Ok(())
    }

    pub(crate) fn get_import(&self, import_source: &ImportSource) -> Result<Value> {
        let loaded_modules = self.0.loaded_modules.borrow();
        loaded_modules
            .get(import_source.module_id())
            .ok_or_else(|| RuntimeError::new_internal_error("Module not found in global context."))?
            .get_export(import_source.import_name())
    }

    pub(crate) fn resolve_instructions(&self, inst_list: &InstructionList) -> Result<InstEvalList> {
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

    pub(super) fn get_init_function(&self, module_id: &ModuleId) -> Result<Option<Function>> {
        let loaded_modules = self.0.loaded_modules.borrow();
        loaded_modules
            .get(module_id)
            .ok_or_else(|| RuntimeError::new_internal_error("Module not found in global context."))?
            .get_init_function()
    }

    pub(super) fn set_module_initialized(&self, module_id: &ModuleId) -> Result<()> {
        let loaded_modules = self.0.loaded_modules.borrow();
        loaded_modules
            .get(module_id)
            .ok_or_else(|| RuntimeError::new_internal_error("Module not found in global context."))?
            .set_is_initialized();
        Ok(())
    }

    pub(super) fn add_top_level_contents(&self, contents: TopLevelContents) {
        self.0
            .top_level_contents
            .borrow_mut()
            .insert(contents.get_ptr(), contents);
    }

    pub(super) fn remove_top_level_contents(&self, contents: TopLevelContents) {
        self.0
            .top_level_contents
            .borrow_mut()
            .remove(&contents.get_ptr());
    }

    pub(super) fn add_eval_context_contents(&self, contents: EvalContextContents) {
        self.0
            .eval_context_contents
            .borrow_mut()
            .insert(contents.get_ptr(), contents);
    }

    pub(super) fn remove_eval_context_contents(&self, contents: EvalContextContents) {
        self.0
            .eval_context_contents
            .borrow_mut()
            .remove(&contents.get_ptr());
    }
}

/// Crate internal methods for global context.
impl GlobalEnv {
    pub(crate) fn create_deferred_ref<T>(&self) -> (GcRef<T>, impl FnOnce(T))
    where
        T: GcTraceable + 'static,
    {
        self.0.gc_context.create_deferred_ref()
    }
}

impl GcTraceable for GlobalEnv {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        self.0.trace(visitor);
    }
}
