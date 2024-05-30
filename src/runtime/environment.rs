use crate::gc::{GcTraceable, PinnedGcRef};

use super::{
    error::{Result, RuntimeError},
    global_env::GlobalEnv,
    value::{PinnedValue, Value},
};

#[derive(Clone)]
pub(crate) struct ModuleImportEnvironment {
    imports: Vec<Value>,
}

impl GcTraceable for ModuleImportEnvironment {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: crate::gc::GcRefVisitor,
    {
        for value in &self.imports {
            value.trace(visitor);
        }
    }
}

impl ModuleImportEnvironment {
    pub fn new(gc_env: &GlobalEnv, imports: Vec<PinnedValue>) -> PinnedGcRef<Self> {
        gc_env.with_lock(|lock| {
            gc_env.create_pinned_ref(ModuleImportEnvironment {
                imports: imports.into_iter().map(|v| v.into_value(lock)).collect(),
            })
        })
    }

    pub fn get_import(&self, index: u32) -> Result<PinnedValue> {
        self.imports
            .get(usize::try_from(index).unwrap())
            .map(Value::pin)
            .ok_or_else(|| RuntimeError::new_internal_error("Import index out of bounds."))
    }
}
