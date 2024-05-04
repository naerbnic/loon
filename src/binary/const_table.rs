use std::rc::Rc;

use crate::{
    pure_values::{Float, Integer},
    runtime::{
        constants::{ConstLoader, ResolveFunc},
        context::ConstResolutionContext,
        environment::ModuleImportEnvironment,
        error::RuntimeError,
        value::{Function, List, Value},
    },
    util::imm_string::ImmString,
};

use super::{error::ValidationError, instructions::InstructionList};

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

    pub fn in_prev_layer(&self) -> Option<Self> {
        if self.layer > 0 {
            Some(LayerIndex {
                layer: self.layer - 1,
                index: self.index,
            })
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub enum ConstIndex {
    /// An index into the stack of constant tables.
    ModuleConst(u32),

    /// An index to be resolved globally by name.
    ModuleImport(u32),
}

impl ConstIndex {
    pub fn resolve(
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

impl ConstLoader for ConstValue {
    fn load<'a>(
        &'a self,
        ctxt: &'a ConstResolutionContext,
    ) -> Result<(crate::runtime::value::Value, ResolveFunc<'a>), RuntimeError> {
        let (value, resolver) = match self {
            ConstValue::Bool(b) => (Value::Bool(*b), None),
            ConstValue::Integer(i) => (Value::Integer(i.clone()), None),
            ConstValue::Float(f) => (Value::Float(f.clone()), None),
            ConstValue::String(s) => (Value::String(s.clone()), None),
            ConstValue::List(list) => {
                let (deferred, resolve_fn) = ctxt.global_context().create_deferred_ref();
                let resolver: ResolveFunc = Box::new(move |imports, vs| {
                    let mut list_elems = Vec::with_capacity(list.len());
                    for index in list {
                        list_elems.push(index.resolve(imports, vs)?);
                    }
                    resolve_fn(List::from_iter(list_elems));
                    Ok(())
                });

                (Value::List(deferred), Some(resolver))
            }
            ConstValue::Function(const_func) => {
                let (deferred, resolve_fn) = ctxt.global_context().create_deferred_ref();
                let resolver: ResolveFunc = Box::new(move |imports, vs| {
                    let module_constants = const_func.module_constants();
                    let mut resolved_func_consts =
                        Vec::with_capacity(const_func.module_constants().len());
                    for index in module_constants {
                        resolved_func_consts.push(index.resolve(imports, vs)?);
                    }
                    resolve_fn(Function::new_managed(
                        resolved_func_consts,
                        Rc::new(
                            ctxt.global_context()
                                .resolve_instructions(const_func.instructions())?,
                        ),
                    ));
                    Ok(())
                });
                (Value::Function(deferred), Some(resolver))
            }
        };

        Ok((value, resolver.unwrap_or(Box::new(|_, _| Ok(())))))
    }
}

/// Check that the constant values are valid, and return the set of constraints
/// the table has to meet.
pub fn validate_const_values(
    table_elements: &[ConstValue],
    globals_size: u32,
    imports_size: u32,
) -> Result<(), ValidationError> {
    let check_index = |index: &ConstIndex| {
        match index {
            ConstIndex::ModuleConst(i) => {
                if *i >= globals_size {
                    return Err(ValidationError::LocalIndexResolutionError);
                }
            }
            ConstIndex::ModuleImport(i) => {
                if *i >= imports_size {
                    return Err(ValidationError::LocalIndexResolutionError);
                }
            }
        }
        Ok(())
    };

    for value in table_elements {
        match value {
            ConstValue::List(list) => {
                for index in list {
                    check_index(index)?;
                }
            }
            ConstValue::Function(_) => {
                // FIXME: Const tables should preserve the enviroment they
                // expect, to allow for validation outside of the context of
                // building the const table.
            }
            _ => {}
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct ConstTable {
    entries: Rc<Vec<ConstValue>>,
}

impl ConstTable {
    pub fn new(values: Vec<ConstValue>) -> Result<Self, ValidationError> {
        Ok(ConstTable {
            entries: Rc::new(values),
        })
    }

    pub fn values(&self) -> &[ConstValue] {
        &self.entries
    }
}
