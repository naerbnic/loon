//! A managed function, representing code within the Loon runtime to evaluate.

use std::rc::Rc;

use crate::{
    refs::{GcRefVisitor, GcTraceable},
    runtime::{constants::ValueTable, instructions::InstEvalList, modules::ModuleGlobals},
};

/// A managed function, representing code within the Loon runtime to evaluate.
pub(crate) struct ManagedFunction {
    globals: ModuleGlobals,
    constants: ValueTable,
    inst_list: Rc<InstEvalList>,
}

impl ManagedFunction {
    pub fn new(globals: ModuleGlobals, constants: ValueTable, inst_list: Rc<InstEvalList>) -> Self {
        ManagedFunction {
            globals,
            constants,
            inst_list,
        }
    }

    pub fn inst_list(&self) -> &Rc<InstEvalList> {
        &self.inst_list
    }

    pub fn constants(&self) -> &ValueTable {
        &self.constants
    }

    pub fn globals(&self) -> &ModuleGlobals {
        &self.globals
    }
}

impl GcTraceable for ManagedFunction {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        self.globals.trace(visitor);
        self.constants.trace(visitor);
    }
}
