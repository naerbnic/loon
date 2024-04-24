use std::rc::Rc;

use crate::{
    refs::{GcRefVisitor, GcTraceable, GcRef},
    runtime::instructions::{InstructionList, StackFrame},
    Value,
};

pub mod native;

pub struct LoonFunction {
    inst_list: Rc<InstructionList>,
}

pub struct Closure {
    function: GcRef<Function>,
    captured_values: Vec<Value>,
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

impl GcTraceable for Function {
    fn trace<V>(&self, _visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        match self {
            Function::Loon(_) => {}
        }
    }
}
