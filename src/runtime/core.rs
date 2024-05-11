use crate::binary::{modules::ModuleId, ConstModule};

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

    pub fn load_module(&self, module_id: ModuleId, module: &ConstModule) -> Result<()> {
        self.global_env.load_module(module_id, module)
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
