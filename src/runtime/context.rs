//! Global contexts for the current state of a runtime environment.

use std::cell::RefCell;

use crate::gc::PinnedGcRef;

use super::{
    constants::ValueTable, environment::ModuleImportEnvironment, error::Result,
    global_env::GlobalEnv, modules::ModuleGlobals, stack_frame::PinnedValueList,
    value::PinnedValue,
};
pub struct ConstResolutionContext<'a> {
    env: &'a GlobalEnv,
    module_globals: &'a PinnedGcRef<ModuleGlobals>,
    import_environment: &'a ModuleImportEnvironment,
}

impl<'a> ConstResolutionContext<'a> {
    pub fn new(
        env: &'a GlobalEnv,
        module_globals: &'a PinnedGcRef<ModuleGlobals>,
        import_environment: &'a ModuleImportEnvironment,
    ) -> Self {
        ConstResolutionContext {
            env,
            module_globals,
            import_environment,
        }
    }

    pub fn env(&self) -> &GlobalEnv {
        self.env
    }

    pub fn module_globals(&self) -> &PinnedGcRef<ModuleGlobals> {
        self.module_globals
    }

    pub fn import_environment(&self) -> &ModuleImportEnvironment {
        self.import_environment
    }
}

pub struct InstEvalContext<'a> {
    global_context: &'a GlobalEnv,
    local_constants: &'a ValueTable,
    globals: &'a ModuleGlobals,
    temp_stack: RefCell<&'a mut PinnedValueList>,
}

impl<'a> InstEvalContext<'a> {
    pub fn new(
        global_context: &'a GlobalEnv,
        local_constants: &'a ValueTable,
        globals: &'a ModuleGlobals,
        temp_stack: &'a mut PinnedValueList,
    ) -> Self {
        InstEvalContext {
            global_context,
            local_constants,
            globals,
            temp_stack: RefCell::new(temp_stack),
        }
    }

    pub fn get_env(&self) -> &GlobalEnv {
        self.global_context
    }

    pub fn get_constant(&self, index: u32) -> Result<PinnedValue> {
        self.local_constants.at(index)
    }

    pub fn get_global(&self, index: u32) -> Result<PinnedValue> {
        self.globals.at(index)
    }

    pub fn set_global(&self, index: u32, value: PinnedValue) -> Result<()> {
        self.globals.set(index, value)
    }

    pub fn with_temp_stack<F, R>(&self, body: F) -> R
    where
        F: FnOnce(&mut PinnedValueList) -> R,
    {
        body(*self.temp_stack.borrow_mut())
    }
}
