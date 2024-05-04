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

use super::instructions::InstructionList;

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
