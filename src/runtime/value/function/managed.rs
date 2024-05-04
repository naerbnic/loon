//! A managed function, representing code within the Loon runtime to evaluate.

use std::rc::Rc;

use crate::runtime::{instructions::InstEvalList, modules::ModuleGlobals, value::Value};

/// A managed function, representing code within the Loon runtime to evaluate.
pub struct ManagedFunction {
    globals: ModuleGlobals,
    constants: Rc<Vec<Value>>,
    inst_list: Rc<InstEvalList>,
}

impl ManagedFunction {
    pub fn new(
        globals: ModuleGlobals,
        constants: Rc<Vec<Value>>,
        inst_list: Rc<InstEvalList>,
    ) -> Self {
        ManagedFunction {
            globals,
            constants,
            inst_list,
        }
    }

    pub fn inst_list(&self) -> &Rc<InstEvalList> {
        &self.inst_list
    }

    pub fn constants(&self) -> &Rc<Vec<Value>> {
        &self.constants
    }
}
