//! A managed function, representing code within the Loon runtime to evaluate.

use std::rc::Rc;

use crate::runtime::{instructions::InstructionList, value::Value};

/// A managed function, representing code within the Loon runtime to evaluate.
pub struct ManagedFunction {
    constants: Rc<Vec<Value>>,
    inst_list: Rc<InstructionList>,
}

impl ManagedFunction {
    pub fn new(constants: Rc<Vec<Value>>, inst_list: Rc<InstructionList>) -> Self {
        ManagedFunction {
            constants,
            inst_list,
        }
    }

    pub fn inst_list(&self) -> &Rc<InstructionList> {
        &self.inst_list
    }

    pub fn constants(&self) -> &Rc<Vec<Value>> {
        &self.constants
    }
}
