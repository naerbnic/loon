use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use crate::binary::{self, modules::ModuleMemberId};

use super::{
    constants::ValueTable,
    context::{ConstResolutionContext, GlobalContext},
    environment::ModuleImportEnvironment,
    error::{Result, RuntimeError},
    value::Value,
};

pub struct ModuleGlobals(Rc<Vec<RefCell<Option<Value>>>>);

impl ModuleGlobals {
    pub fn from_size_empty(size: u32) -> Self {
        let mut globals = Vec::with_capacity(usize::try_from(size).unwrap());
        for _ in 0..size {
            globals.push(RefCell::new(None));
        }
        ModuleGlobals(Rc::new(globals))
    }
}

struct Inner {
    members: ValueTable,
    module_globals: ModuleGlobals,
    exports: HashMap<ModuleMemberId, u32>,
    initializer: Option<u32>,
    is_initialized: Cell<bool>,
}

pub struct Module(Rc<Inner>);

impl Module {
    pub fn from_binary(
        ctxt: &GlobalContext,
        module: &binary::modules::ConstModule,
    ) -> Result<Self> {
        // Resolve imports
        let import_values = module
            .imports()
            .iter()
            .map(|id| ctxt.get_import(id))
            .collect::<Result<Vec<_>>>()?;
        let import_env = ModuleImportEnvironment::new(import_values);
        let const_ctxt = ConstResolutionContext::new_with_imports(ctxt.clone(), import_env);
        let members = ValueTable::from_binary(module.const_table(), &const_ctxt)?;
        let module_globals = ModuleGlobals::from_size_empty(module.global_table_size());
        // The module is already initialized if there is no initializer to run.
        let is_initialized = module.initializer().is_none();
        Ok(Module(Rc::new(Inner {
            members,
            module_globals,
            exports: module.exports().clone(),
            initializer: module.initializer(),
            is_initialized: Cell::new(is_initialized),
        })))
    }

    pub fn get_export(&self, name: &ModuleMemberId) -> Result<Value> {
        let index = self
            .0
            .exports
            .get(name)
            .ok_or_else(|| RuntimeError::new_internal_error("Export not found."))?;
        self.0.members.at(*index).cloned()
    }
}
