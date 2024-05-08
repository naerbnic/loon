use crate::binary::{modules::ModuleId, ConstModule};

use super::{error::Result, global_env::GlobalEnv, TopLevelRuntime};

pub struct Runtime {
    global_env: GlobalEnv,
}

impl Runtime {
    pub fn new() -> Self {
        Runtime {
            global_env: GlobalEnv::new(),
        }
    }

    /// Loads a module into this runtime.
    ///
    /// This does not initialize the module state, and has to be done at a
    /// later pass.
    pub fn load_module(&self, module_id: ModuleId, module: &ConstModule) -> Result<()> {
        self.global_env.load_module(module_id, module)
    }

    pub fn make_top_level(&self) -> TopLevelRuntime {
        TopLevelRuntime::new(self.global_env.clone())
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
