use crate::{
    pure_values::{Float, Integer},
    runtime::{environment::ModuleImportEnvironment, error::RuntimeError, value::Value},
    util::imm_string::ImmString,
};

use super::instructions::InstructionList;

#[derive(Clone, Debug)]
pub enum ConstIndex {
    /// An index into the stack of constant tables.
    ModuleConst(u32),

    /// An index to be resolved globally by name.
    ModuleImport(u32),
}

impl ConstIndex {
    pub(crate) fn resolve(
        &self,
        imports: &ModuleImportEnvironment,
        consts: &[Value],
    ) -> Result<Value, RuntimeError> {
        match self {
            ConstIndex::ModuleConst(index) => consts
                .get(usize::try_from(*index).unwrap())
                .cloned()
                .ok_or_else(|| RuntimeError::new_internal_error("Invalid index.")),
            ConstIndex::ModuleImport(index) => imports.get_import(*index),
        }
    }

    pub fn as_module_const(&self) -> Option<u32> {
        match self {
            ConstIndex::ModuleConst(index) => Some(*index),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConstFunction {
    /// Definitions of constants local to the function.
    module_constants: Vec<ConstIndex>,
    instructions: InstructionList,
}

impl ConstFunction {
    pub fn new(module_constants: Vec<ConstIndex>, instructions: InstructionList) -> Self {
        ConstFunction {
            module_constants,
            instructions,
        }
    }

    pub fn module_constants(&self) -> &[ConstIndex] {
        &self.module_constants[..]
    }

    pub fn instructions(&self) -> &InstructionList {
        &self.instructions
    }
}

#[derive(Clone, Debug)]
pub enum ConstValue {
    Bool(bool),
    Integer(Integer),
    Float(Float),
    String(ImmString),
    List(Vec<ConstIndex>),
    Function(ConstFunction),
}
