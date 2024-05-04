use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use crate::binary::{self, modules::ModuleMemberId};

use super::{
    constants::ValueTable,
    context::{ConstResolutionContext, GlobalEnv},
    environment::ModuleImportEnvironment,
    error::{Result, RuntimeError},
    value::Value,
};

#[derive(Clone)]
pub struct ModuleGlobals(Rc<Vec<RefCell<Option<Value>>>>);

impl ModuleGlobals {
    pub fn from_size_empty(size: u32) -> Self {
        let mut globals = Vec::with_capacity(usize::try_from(size).unwrap());
        for _ in 0..size {
            globals.push(RefCell::new(None));
        }
        ModuleGlobals(Rc::new(globals))
    }

    pub fn at(&self, index: u32) -> Result<Value> {
        let cell = self
            .0
            .get(usize::try_from(index).unwrap())
            .ok_or_else(|| RuntimeError::new_internal_error("Index out of bounds."))?;
        cell.borrow()
            .clone()
            .ok_or_else(|| RuntimeError::new_internal_error("Global not set."))
    }

    pub fn set(&self, index: u32, value: Value) -> std::prelude::v1::Result<(), RuntimeError> {
        let mut cell = self
            .0
            .get(usize::try_from(index).unwrap())
            .ok_or_else(|| RuntimeError::new_internal_error("Index out of bounds."))?
            .borrow_mut();
        cell.replace(value);
        Ok(())
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
        ctxt: &GlobalEnv,
        module: &binary::modules::ConstModule,
    ) -> Result<Self> {
        // Resolve imports
        let import_values = module
            .imports()
            .iter()
            .map(|id| ctxt.get_import(id))
            .collect::<Result<Vec<_>>>()?;
        let module_globals = ModuleGlobals::from_size_empty(module.global_table_size());
        let import_env = ModuleImportEnvironment::new(import_values);
        let const_ctxt = ConstResolutionContext::new(ctxt, &module_globals, &import_env);
        let members = ValueTable::from_binary(module.const_table(), &const_ctxt)?;
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
