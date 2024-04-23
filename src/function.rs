use std::rc::Rc;

use crate::{
    runtime::instructions::{InstructionList, StackFrame},
    Value,
};

pub mod native;

pub struct LoonFunction {
    inst_list: Rc<InstructionList>,
}

pub enum Function {
    Loon(LoonFunction),
}

impl Function {
    pub fn new_loon(inst_list: Rc<InstructionList>) -> Self {
        Function::Loon(LoonFunction { inst_list })
    }

    pub fn make_stack_frame(&self, args: Vec<Value>) -> StackFrame {
        match self {
            Function::Loon(loon_func) => StackFrame::new(loon_func.inst_list.clone(), args),
        }
    }
}
