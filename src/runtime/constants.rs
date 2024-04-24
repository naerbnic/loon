//! Loon has constants that represent constant values that can be resolved at
//! runtime. They don't themselves refer to Values, as that would require the
//! presence of a runtime, but they can be used to create Values.

use std::rc::Rc;

use super::{instructions::InstructionList, value::{Float, Integer}};

#[derive(Clone, Debug)]
pub enum ConstIndex {
    Local(usize),
    Global(Rc<String>),
}

#[derive(Clone, Debug)]
pub struct ConstFunction {
    const_table: Vec<ConstIndex>,
    instructions: Rc<InstructionList>,
}

#[derive(Clone, Debug)]
pub enum ConstValue {
    Integer(Integer),
    Float(Float),
    String(String),
    List(Vec<ConstIndex>),
    Function(ConstFunction),
}

pub trait ConstResolver {
    fn resolve(&self, index: &ConstIndex);
}
