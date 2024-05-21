use crate::binary::{module_set::ModuleSet, ConstModule};

use super::{error::Result, global_env::GlobalEnv, TopLevelRuntime};

pub struct Runtime {
    global_env: GlobalEnv,
}

impl Runtime {
    #[must_use]
    pub fn new() -> Self {
        Runtime {
            global_env: GlobalEnv::new(),
        }
    }

    pub fn load_module(&self, module: &ConstModule) -> Result<()> {
        self.global_env.load_module(module)
    }

    pub fn load_module_set(&self, module_set: &ModuleSet) -> Result<()> {
        if !module_set
            .external_dependencies()
            .all(|module_id| self.global_env.is_module_loaded(module_id))
        {
            panic!("Dependency not satisfied.");
        }

        // FIXME: This is a naive implementation that does not handle
        // dependencies correctly.
        for module in module_set.modules() {
            self.load_module(module)?;
        }
        Ok(())
    }

    #[must_use]
    pub fn make_top_level(&self) -> TopLevelRuntime {
        TopLevelRuntime::new(self.global_env.clone())
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
