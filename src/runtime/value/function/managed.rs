//! A managed function, representing code within the Loon runtime to evaluate.

use std::{cell::OnceCell, rc::Rc};

use crate::{
    gc::{GcRefVisitor, GcTraceable},
    runtime::{
        constants::{self, ValueTable},
        instructions::InstEvalList,
        modules::ModuleGlobals,
    },
};

/// A managed function, representing code within the Loon runtime to evaluate.
pub(crate) struct ManagedFunction {
    globals: ModuleGlobals,
    constants: OnceCell<ValueTable>,
    inst_list: Rc<InstEvalList>,
}

impl ManagedFunction {
    pub fn new(globals: ModuleGlobals, constants: ValueTable, inst_list: Rc<InstEvalList>) -> Self {
        ManagedFunction {
            globals,
            constants: constants.into(),
            inst_list,
        }
    }

    pub fn new_deferred(globals: ModuleGlobals, inst_list: Rc<InstEvalList>) -> Self {
        ManagedFunction {
            globals,
            constants: OnceCell::new(),
            inst_list,
        }
    }

    pub fn inst_list(&self) -> &Rc<InstEvalList> {
        &self.inst_list
    }

    pub fn constants(&self) -> &ValueTable {
        self.constants.get().expect("Constants not resolved.")
    }

    pub fn resolve_constants(&self, constants: ValueTable) {
        let result = self.constants.set(constants);
        assert!(result.is_ok(), "Constants already resolved.");
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
        if let Some(constants) = self.constants.get() {
            constants.trace(visitor);
        }
    }
}
