use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::binary::{self, modules::ModuleMemberId};

use super::{
    constants::ValueTable,
    context::GlobalContext,
    error::{Result, RuntimeError},
    value::Value,
};

pub struct ModuleGlobals(Rc<Vec<RefCell<Option<Value>>>>);

struct Inner {
    members: ValueTable,
    module_globals: ModuleGlobals,
    exports: HashMap<ModuleMemberId, u32>,
}

pub struct Module(Rc<Inner>);

impl Module {
    pub fn from_binary(
        ctxt: &GlobalContext,
        module: &binary::modules::ConstModule,
    ) -> Result<Self> {
        todo!()
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
