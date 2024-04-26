//! A managed function, representing code within the Loon runtime to evaluate.

use std::rc::Rc;

use crate::runtime::{instructions::InstEvalList, value::Value};

/// A managed function, representing code within the Loon runtime to evaluate.
pub struct ManagedFunction {
    constants: Rc<Vec<Value>>,
    inst_list: Rc<InstEvalList>,
}

impl ManagedFunction {
    pub fn new(constants: Rc<Vec<Value>>, inst_list: Rc<InstEvalList>) -> Self {
        ManagedFunction {
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
