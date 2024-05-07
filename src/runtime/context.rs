//! Global contexts for the current state of a runtime environment.

use super::{
    constants::ValueTable, environment::ModuleImportEnvironment, error::Result,
    global_env::GlobalEnv, modules::ModuleGlobals, value::Value,
};
pub struct ConstResolutionContext<'a> {
    global_context: &'a GlobalEnv,
    module_globals: &'a ModuleGlobals,
    import_environment: &'a ModuleImportEnvironment,
}

impl<'a> ConstResolutionContext<'a> {
    pub fn new(
        global_context: &'a GlobalEnv,
        module_globals: &'a ModuleGlobals,
        import_environment: &'a ModuleImportEnvironment,
    ) -> Self {
        ConstResolutionContext {
            global_context,
            module_globals,
            import_environment,
        }
    }

    pub fn global_context(&self) -> &GlobalEnv {
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
    global_context: &'a GlobalEnv,
    local_constants: &'a ValueTable,
    globals: &'a ModuleGlobals,
}

impl<'a> InstEvalContext<'a> {
    pub fn new(
        global_context: &'a GlobalEnv,
        local_constants: &'a ValueTable,
        globals: &'a ModuleGlobals,
    ) -> Self {
        InstEvalContext {
            global_context,
            local_constants,
            globals,
        }
    }

    pub fn get_env(&self) -> &GlobalEnv {
        self.global_context
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
