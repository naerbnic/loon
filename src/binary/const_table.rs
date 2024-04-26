use std::rc::Rc;

use crate::pure_values::{Float, Integer};

use super::{instructions::InstructionList, symbols::GlobalSymbol};

/// An index of a constant in the layers of constant values.
///
/// The layer is relative to the current context, with 0 being the current
/// context, 1 being the parent context, and so on.
///
/// The index is the index in the specified layer's values.
#[derive(Clone, Debug)]
pub struct LayerIndex {
    layer: usize,
    index: usize,
}

impl LayerIndex {
    pub fn new(layer: usize, index: usize) -> Self {
        LayerIndex { layer, index }
    }

    #[cfg(test)]
    pub fn new_in_base(index: usize) -> Self {
        LayerIndex { layer: 0, index }
    }

    pub fn layer(&self) -> usize {
        self.layer
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

#[derive(Clone, Debug)]
pub enum ConstIndex {
    /// An index into the stack of constant tables.
    Local(LayerIndex),

    /// An index to be resolved globally by name.
    Global(GlobalSymbol),
}

#[derive(Clone, Debug)]
pub struct ConstFunction {
    /// Definitions of constants local to the function.
    const_table: Vec<ConstValue>,
    instructions: Rc<InstructionList>,
}

impl ConstFunction {
    pub fn new(const_table: Vec<ConstValue>, instructions: Rc<InstructionList>) -> Self {
        ConstFunction {
            const_table,
            instructions,
        }
    }

    pub fn const_table(&self) -> &[ConstValue] {
        &self.const_table
    }

    pub fn instructions(&self) -> &InstructionList {
        &self.instructions
    }
}

#[derive(Clone, Debug)]
pub enum ConstValue {
    /// An external ref to a constant.
    ///
    /// The resolution layer starts with the parent, so a layer of 0 refers to
    /// the parent context.
    ExternalRef(ConstIndex),
    Integer(Integer),
    Float(Float),
    String(String),
    List(Vec<ConstIndex>),
    Function(ConstFunction),
}
