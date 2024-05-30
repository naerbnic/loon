use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use super::{
    constants::ValueTable,
    context::ConstResolutionContext,
    environment::ModuleImportEnvironment,
    error::{Result, RuntimeError},
    global_env::GlobalEnv,
    value::{Function, PinnedValue, Value},
};
use crate::{
    binary::{modules::ModuleMemberId, ConstModule},
    gc::{GcRef, GcTraceable, PinnedGcRef},
};

pub struct ModuleGlobals {
    values: Vec<RefCell<Option<Value>>>,
}

impl ModuleGlobals {
    pub fn from_size_empty(global_env: &GlobalEnv, size: u32) -> PinnedGcRef<Self> {
        let mut globals = Vec::with_capacity(usize::try_from(size).unwrap());
        for _ in 0..size {
            globals.push(RefCell::new(None));
        }
        global_env.create_pinned_ref(ModuleGlobals { values: globals })
    }

    pub fn at(&self, index: u32) -> Result<PinnedValue> {
        let cell = self
            .values
            .get(usize::try_from(index).unwrap())
            .ok_or_else(|| RuntimeError::new_internal_error("Index out of bounds."))?;
        let result = cell
            .borrow()
            .as_ref()
            .map(Value::pin)
            .ok_or_else(|| RuntimeError::new_internal_error("Global not set."))?;
        Ok(result)
    }

    pub fn set(
        &self,
        index: u32,
        value: PinnedValue,
    ) -> std::prelude::v1::Result<(), RuntimeError> {
        let mut cell = self
            .values
            .get(usize::try_from(index).unwrap())
            .ok_or_else(|| RuntimeError::new_internal_error("Index out of bounds."))?
            .borrow_mut();
        cell.replace(value.to_value());
        Ok(())
    }
}

impl GcTraceable for ModuleGlobals {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: crate::gc::GcRefVisitor,
    {
        for value in &self.values {
            if let Some(value) = &*value.borrow() {
                value.trace(visitor);
            }
        }
    }
}

pub struct Module {
    members: GcRef<ValueTable>,
    module_globals: GcRef<ModuleGlobals>,
    exports: HashMap<ModuleMemberId, u32>,
    initializer: Option<u32>,
    is_initialized: Cell<bool>,
}

impl Module {
    pub fn from_binary(ctxt: &GlobalEnv, module: &ConstModule) -> Result<PinnedGcRef<Self>> {
        // Resolve imports
        let import_values = module
            .imports()
            .iter()
            .map(|id| ctxt.get_import(id))
            .collect::<Result<Vec<_>>>()?;
        let module_globals = ModuleGlobals::from_size_empty(ctxt, module.global_table_size());
        let import_env = ModuleImportEnvironment::new(ctxt, import_values);
        let members = {
            let const_ctxt = ConstResolutionContext::new(ctxt, &module_globals, &import_env);
            ValueTable::from_binary(module.const_table(), &const_ctxt)?
        };
        // The module is already initialized if there is no initializer to run.
        let is_initialized = module.initializer().is_none();
        let lock = ctxt.lock_collect();
        Ok(ctxt.create_pinned_ref(Module {
            members: members.into_ref(lock.guard()),
            module_globals: module_globals.into_ref(lock.guard()),
            exports: module.exports().clone(),
            initializer: module.initializer(),
            is_initialized: Cell::new(is_initialized),
        }))
    }

    pub fn get_export(&self, name: &ModuleMemberId) -> Result<PinnedValue> {
        let index = self
            .exports
            .get(name)
            .ok_or_else(|| RuntimeError::new_internal_error("Export not found."))?;
        self.members.borrow().at(*index).map(Value::pin)
    }

    pub fn get_init_function(&self) -> Result<Option<GcRef<Function>>> {
        if self.is_initialized.get() {
            return Ok(None);
        }
        let index = self
            .initializer
            .expect("Can only be uninitialized if there is an initializer.");
        Ok(Some(
            self.members.borrow().at(index)?.as_function()?.clone(),
        ))
    }

    pub fn set_is_initialized(&self) {
        self.is_initialized.set(true);
    }
}

impl GcTraceable for Module {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: crate::gc::GcRefVisitor,
    {
        self.module_globals.trace(visitor);
        self.members.trace(visitor);
    }
}
