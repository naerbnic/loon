use std::rc::Rc;

use super::{
    error::{Result, RuntimeError},
    value::Value,
};

/// The environment for functions defined within a module.
struct ModuleEnvironmentInner {
    imports: Vec<Value>,
}

#[derive(Clone)]
pub struct ModuleImportEnvironment(Rc<ModuleEnvironmentInner>);

impl ModuleImportEnvironment {
    pub fn new(imports: Vec<Value>) -> Self {
        ModuleImportEnvironment(Rc::new(ModuleEnvironmentInner {
            imports,
        }))
    }

    pub fn get_import(&self, index: u32) -> Result<Value> {
        self.0
            .imports
            .get(usize::try_from(index).unwrap())
            .cloned()
            .ok_or_else(|| RuntimeError::new_internal_error("Import index out of bounds."))
    }
}
