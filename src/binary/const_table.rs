use std::rc::Rc;

use crate::{
    pure_values::{Float, Integer},
    runtime::{
        constants::{resolve_constants, ConstLoader, ResolveFunc},
        value::{Function, List, Value},
    },
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

impl ConstLoader for ConstValue {
    fn load<'a>(
        &'a self,
        const_resolver: &'a dyn crate::runtime::constants::ConstResolver,
        ctxt: &'a crate::runtime::context::GlobalContext,
    ) -> Result<(crate::runtime::value::Value, ResolveFunc<'a>), crate::runtime::error::RuntimeError>
    {
        type ResolverFn<'a> = Box<
            dyn FnOnce(
                    &dyn crate::runtime::constants::ConstResolver,
                ) -> Result<(), crate::runtime::error::RuntimeError>
                + 'a,
        >;
        let (value, resolver) = match self {
            ConstValue::ExternalRef(index) => (const_resolver.resolve(index)?, None),
            ConstValue::Integer(i) => (Value::Integer(i.clone()), None),
            ConstValue::Float(f) => (Value::Float(f.clone()), None),
            ConstValue::String(s) => (Value::String(Rc::new(s.clone())), None),
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
                        resolve_constants(ctxt, vs, const_func.const_table())?;
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
