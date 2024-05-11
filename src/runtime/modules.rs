use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use super::{
    constants::ValueTable,
    context::ConstResolutionContext,
    environment::ModuleImportEnvironment,
    error::{Result, RuntimeError},
    global_env::GlobalEnvLock,
    value::{Function, Value},
};
use crate::{
    binary::{modules::ModuleMemberId, ConstModule},
    gc::{GcRef, GcTraceable},
};

pub struct ModuleGlobalsInner {
    values: Vec<RefCell<Option<Value>>>,
}

impl GcTraceable for ModuleGlobalsInner {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: crate::gc::GcRefVisitor,
    {
        for value in self.values.iter() {
            if let Some(value) = &*value.borrow() {
                value.trace(visitor);
            }
        }
    }
}

#[derive(Clone)]
pub struct ModuleGlobals(GcRef<ModuleGlobalsInner>);

impl ModuleGlobals {
    pub fn from_size_empty(global_env: &GlobalEnvLock, size: u32) -> Self {
        let mut globals = Vec::with_capacity(usize::try_from(size).unwrap());
        for _ in 0..size {
            globals.push(RefCell::new(None));
        }
        ModuleGlobals(global_env.create_ref(ModuleGlobalsInner { values: globals }))
    }

    pub fn at(&self, index: u32) -> Result<Value> {
        let globals = self.0.borrow();
        let cell = globals
            .values
            .get(usize::try_from(index).unwrap())
            .ok_or_else(|| RuntimeError::new_internal_error("Index out of bounds."))?;
        let result = cell
            .borrow()
            .clone()
            .ok_or_else(|| RuntimeError::new_internal_error("Global not set."))?;
        Ok(result)
    }

    pub fn set(&self, index: u32, value: Value) -> std::prelude::v1::Result<(), RuntimeError> {
        let globals = self.0.borrow();
        let mut cell = globals
            .values
            .get(usize::try_from(index).unwrap())
            .ok_or_else(|| RuntimeError::new_internal_error("Index out of bounds."))?
            .borrow_mut();
        cell.replace(value);
        Ok(())
    }
}

impl GcTraceable for ModuleGlobals {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: crate::gc::GcRefVisitor,
    {
        visitor.visit(&self.0);
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
    pub fn from_binary(ctxt: &GlobalEnvLock, module: &ConstModule) -> Result<Self> {
        // Resolve imports
        let import_values = module
            .imports()
            .iter()
            .map(|id| ctxt.get_import(id))
            .collect::<Result<Vec<_>>>()?;
        let module_globals = ModuleGlobals::from_size_empty(ctxt, module.global_table_size());
        let import_env = ModuleImportEnvironment::new(import_values);
        let members = {
            let const_ctxt = ConstResolutionContext::new(ctxt, &module_globals, &import_env);
            ValueTable::from_binary(module.const_table(), &const_ctxt)?
        };
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

    pub fn get_init_function(&self) -> Result<Option<GcRef<Function>>> {
        if self.0.is_initialized.get() {
            return Ok(None);
        }
        let index = self
            .0
            .initializer
            .expect("Can only be uninitialized if there is an initializer.");
        Ok(Some(self.0.members.at(index)?.as_function()?.clone()))
    }

    pub fn set_is_initialized(&self) {
        self.0.is_initialized.set(true);
    }
}

impl GcTraceable for Module {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: crate::gc::GcRefVisitor,
    {
        self.0.module_globals.trace(visitor);
        self.0.members.trace(visitor);
    }
}
