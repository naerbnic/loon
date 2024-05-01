use std::rc::Rc;

use crate::{
    pure_values::{Float, Integer},
    runtime::{
        constants::{resolve_constants, ConstLoader, ConstResolver, ResolveFunc},
        context::GlobalContext,
        error::RuntimeError,
        value::{Function, List, Value},
    },
    util::imm_string::ImmString,
};

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
    Local(LayerIndex),

    /// An index to be resolved globally by name.
    Global(GlobalSymbol),
}

#[derive(Clone, Debug)]
pub struct ConstFunction {
    /// Definitions of constants local to the function.
    const_table: ConstTable,
    instructions: InstructionList,
}

impl ConstFunction {
    pub fn new(const_table: ConstTable, instructions: InstructionList) -> Self {
        ConstFunction {
            const_table,
            instructions,
        }
    }

    pub fn const_table(&self) -> &ConstTable {
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
    String(ImmString),
    List(Vec<ConstIndex>),
    Function(ConstFunction),
}

impl ConstLoader for ConstValue {
    fn load<'a>(
        &'a self,
        const_resolver: &'a dyn ConstResolver,
        ctxt: &'a GlobalContext,
    ) -> Result<(crate::runtime::value::Value, ResolveFunc<'a>), RuntimeError> {
        type ResolverFn<'a> = Box<dyn FnOnce(&dyn ConstResolver) -> Result<(), RuntimeError> + 'a>;
        let (value, resolver) = match self {
            ConstValue::ExternalRef(index) => (const_resolver.resolve(index)?, None),
            ConstValue::Integer(i) => (Value::Integer(i.clone()), None),
            ConstValue::Float(f) => (Value::Float(f.clone()), None),
            ConstValue::String(s) => (Value::String(s.clone()), None),
            ConstValue::List(list) => {
                let (deferred, resolve_fn) = ctxt.create_deferred_ref();
                let resolver: ResolverFn = Box::new(move |vs| {
                    let mut list_elems = Vec::with_capacity(list.len());
                    for index in list {
                        list_elems.push(vs.resolve(index)?);
                    }
                    resolve_fn(List::from_iter(list_elems));
                    Ok(())
                });

                (Value::List(deferred), Some(resolver))
            }
            ConstValue::Function(const_func) => {
                let (deferred, resolve_fn) = ctxt.create_deferred_ref();
                let resolver: ResolverFn = Box::new(move |vs| {
                    let resolved_func_consts =
                        resolve_constants(ctxt, vs, const_func.const_table().values())?;
                    resolve_fn(Function::new_managed(
                        resolved_func_consts,
                        Rc::new(ctxt.resolve_instructions(const_func.instructions())?),
                    ));
                    Ok(())
                });
                (Value::Function(deferred), Some(resolver))
            }
        };

        Ok((value, resolver.unwrap_or(Box::new(|_| Ok(())))))
    }
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum ConstValidationError {
    #[error("Found an invalid constant index")]
    LocalIndexResolutionError,
}

pub trait ConstTableEnv {
    fn validate_ref(&self, index: &ConstIndex) -> Result<(), ConstValidationError>;
}

fn validate_const_table(
    env: &dyn ConstTableEnv,
    table_elements: &[ConstValue],
) -> Result<(), ConstValidationError> {
    struct LayerEnv<'a> {
        parent: &'a dyn ConstTableEnv,
        layer_size: usize,
    }

    impl ConstTableEnv for LayerEnv<'_> {
        fn validate_ref(&self, index: &ConstIndex) -> Result<(), ConstValidationError> {
            match index {
                ConstIndex::Local(layer_index) => {
                    if let Some(prev_layer_index) = layer_index.in_prev_layer() {
                        self.parent
                            .validate_ref(&ConstIndex::Local(prev_layer_index))
                    } else if layer_index.index() < self.layer_size {
                        Ok(())
                    } else {
                        Err(ConstValidationError::LocalIndexResolutionError)
                    }
                }
                ConstIndex::Global(_) => self.parent.validate_ref(index),
            }
        }
    }

    let local_value_env = LayerEnv {
        parent: env,
        layer_size: table_elements.len(),
    };

    for value in table_elements {
        match value {
            ConstValue::ExternalRef(index) => {
                // External refs are validated by the parent environment.
                env.validate_ref(index)?
            }
            ConstValue::List(list) => {
                for index in list {
                    local_value_env.validate_ref(index)?;
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
pub struct ConstTable(Rc<Vec<ConstValue>>);

impl ConstTable {
    pub fn new(values: Vec<ConstValue>) -> Result<Self, ConstValidationError> {
        struct NullEnv;

        impl ConstTableEnv for NullEnv {
            fn validate_ref(&self, _: &ConstIndex) -> Result<(), ConstValidationError> {
                Err(ConstValidationError::LocalIndexResolutionError)
            }
        }
        ConstTable::new_with_env(&NullEnv, values)
    }

    pub fn new_with_env(
        env: &dyn ConstTableEnv,
        values: Vec<ConstValue>,
    ) -> Result<Self, ConstValidationError> {
        validate_const_table(env, &values)?;
        Ok(ConstTable(Rc::new(values)))
    }

    pub fn values(&self) -> &[ConstValue] {
        &self.0
    }
}
