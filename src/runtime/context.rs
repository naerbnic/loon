//! Global contexts for the current state of a runtime environment.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use super::{
    constants::ValueTable,
    environment::ModuleImportEnvironment,
    error::{Result, RuntimeError},
    inst_set::{Add, CallDynamic, Pop, PushConst, PushGlobal, ReturnDynamic, SetGlobal},
    instructions::{InstEvalList, InstPtr},
    modules::{Module, ModuleGlobals},
    value::Value,
};
use crate::{
    binary::{
        self,
        instructions::{Instruction, InstructionList},
        modules::{ImportSource, ModuleId},
    },
    refs::{GcContext, GcRef, GcTraceable},
};

struct Inner {
    gc_context: GcContext,
    loaded_modules: RefCell<HashMap<ModuleId, Module>>,
}

#[derive(Clone)]
pub struct GlobalContext(Rc<Inner>);

impl GlobalContext {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        GlobalContext(Rc::new(Inner {
            gc_context: GcContext::new(),
            loaded_modules: RefCell::new(HashMap::new()),
        }))
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

    pub fn get_import(&self, import_source: &ImportSource) -> Result<Value> {
        let loaded_modules = self.0.loaded_modules.borrow();
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
                    Instruction::PushCopy(_) => todo!(),
                    Instruction::PushGlobal(i) => InstPtr::new(PushGlobal::new(*i)),
                    Instruction::PopGlobal(i) => InstPtr::new(SetGlobal::new(*i)),
                    Instruction::Pop(i) => InstPtr::new(Pop::new(*i)),
                    Instruction::Add => InstPtr::new(Add),
                    Instruction::BoolAnd => todo!(),
                    Instruction::BoolOr => todo!(),
                    Instruction::BoolXor => todo!(),
                    Instruction::BoolNot => todo!(),
                    Instruction::Compare(_) => todo!(),
                    Instruction::Branch(_) => todo!(),
                    Instruction::BranchIf(_) => todo!(),
                    Instruction::Call(_) => todo!(),
                    Instruction::CallDynamic => InstPtr::new(CallDynamic),
                    Instruction::Return(_) => todo!(),
                    Instruction::ReturnDynamic => InstPtr::new(ReturnDynamic),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(InstEvalList::from_inst_ptrs(result))
    }
}

/// Crate internal methods for global context.
impl GlobalContext {
    pub(crate) fn create_deferred_ref<T>(&self) -> (GcRef<T>, impl FnOnce(T))
    where
        T: GcTraceable + 'static,
    {
        self.0.gc_context.create_deferred_ref()
    }
}

pub struct ConstResolutionContext<'a> {
    global_context: &'a GlobalContext,
    module_globals: &'a ModuleGlobals,
    import_environment: &'a ModuleImportEnvironment,
}

impl<'a> ConstResolutionContext<'a> {
    pub fn new(
        global_context: &'a GlobalContext,
        module_globals: &'a ModuleGlobals,
        import_environment: &'a ModuleImportEnvironment,
    ) -> Self {
        ConstResolutionContext {
            global_context,
            module_globals,
            import_environment,
        }
    }

    pub fn global_context(&self) -> &GlobalContext {
        self.global_context
    }

    pub fn module_globals(&self) -> &ModuleGlobals {
        self.module_globals
    }

    pub fn import_environment(&self) -> &ModuleImportEnvironment {
        self.import_environment
    }
}

pub struct InstEvalContext<'a> {
    global_context: &'a GlobalContext,
    local_constants: &'a ValueTable,
    globals: &'a ModuleGlobals,
}

impl<'a> InstEvalContext<'a> {
    pub fn new(
        global_context: &'a GlobalContext,
        local_constants: &'a ValueTable,
        globals: &'a ModuleGlobals,
    ) -> Self {
        InstEvalContext {
            global_context,
            local_constants,
            globals,
        }
    }

    pub fn get_constant(&self, index: u32) -> Result<Value> {
        self.local_constants.at(index).cloned()
    }

    pub fn get_global(&self, index: u32) -> Result<Value> {
        self.globals.at(index)
    }

    pub fn set_global(&self, index: u32, value: Value) -> Result<()> {
        self.globals.set(index, value)
    }
}
