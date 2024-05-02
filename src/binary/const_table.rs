use std::{collections::HashSet, rc::Rc};

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

impl ConstIndex {
    pub fn in_prev_layer(&self) -> Option<Self> {
        match self {
            ConstIndex::Local(layer_index) => layer_index.in_prev_layer().map(ConstIndex::Local),
            ConstIndex::Global(g) => Some(ConstIndex::Global(g.clone())),
        }
    }
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

/// Check that the constant values are valid, and return the set of constraints
/// the table has to meet.
fn collect_constraints(
    table_elements: &[ConstValue],
) -> Result<ConstConstraints, ConstValidationError> {
    fn add_local_constraint(
        constraints: &mut ConstConstraints,
        curr_layer_size: usize,
        layer_index: &ConstIndex,
    ) -> Result<(), ConstValidationError> {
        match layer_index {
            ConstIndex::Local(layer_index) => {
                if let Some(prev_layer) = layer_index.in_prev_layer() {
                    constraints.absorb_constraint(&ConstIndex::Local(prev_layer));
                } else if curr_layer_size <= layer_index.index() {
                    return Err(ConstValidationError::LocalIndexResolutionError);
                }
            }
            ConstIndex::Global(global) => {
                constraints.absorb_constraint(&ConstIndex::Global(global.clone()));
            }
        }
        Ok(())
    }

    let mut constraints = ConstConstraints::new();
    for value in table_elements {
        match value {
            ConstValue::ExternalRef(index) => {
                constraints.absorb_constraint(index);
            }
            ConstValue::List(list) => {
                for index in list {
                    add_local_constraint(&mut constraints, table_elements.len(), index)?;
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
    Ok(constraints)
}

#[derive(Debug)]
pub struct ConstConstraints {
    /// This contains constraints for the dependencies of the const table.
    /// There is one entry in the vec is one layer below the const table itself.
    /// The value of the entry is the minimum length of the list of constants
    /// that the const table depends on.
    layer_index_constraints: Vec<u32>,
    global_constraints: HashSet<GlobalSymbol>,
}

impl ConstConstraints {
    pub fn new() -> Self {
        ConstConstraints {
            layer_index_constraints: Vec::new(),
            global_constraints: HashSet::new(),
        }
    }

    pub fn absorb_constraint(&mut self, index: &ConstIndex) {
        match index {
            ConstIndex::Local(layer_index) => {
                if layer_index.layer() >= self.layer_index_constraints.len() {
                    self.layer_index_constraints
                        .resize(layer_index.layer() + 1, 0);
                }
                let layer_constraint = &mut self.layer_index_constraints[layer_index.layer()];
                *layer_constraint = (*layer_constraint).max(layer_index.index() as u32);
            }
            ConstIndex::Global(symbol) => {
                self.global_constraints.insert(symbol.clone());
            }
        }
    }

    pub fn needs_parent_layers(&self) -> bool {
        !self.layer_index_constraints.is_empty()
    }
}

impl Default for ConstConstraints {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct ConstTable {
    entries: Rc<Vec<ConstValue>>,
    constraints: Rc<ConstConstraints>,
}

impl ConstTable {
    pub fn new(values: Vec<ConstValue>) -> Result<Self, ConstValidationError> {
        let constraints = collect_constraints(&values)?;
        Ok(ConstTable {
            constraints: Rc::new(constraints),
            entries: Rc::new(values),
        })
    }

    pub fn values(&self) -> &[ConstValue] {
        &self.entries
    }

    pub fn constraints(&self) -> &ConstConstraints {
        &self.constraints
    }
}
