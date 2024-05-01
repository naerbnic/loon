use std::collections::HashMap;

use crate::binary::symbols::GlobalSymbol;

use super::{constants::ConstTable, context::GlobalContext, error::RuntimeError, Runtime};

pub struct GlobalSymbolSet {
    const_table: ConstTable,
    global_symbol_indexes: HashMap<GlobalSymbol, usize>,
}

impl GlobalSymbolSet {
    pub fn new(const_table: ConstTable, global_symbols: HashMap<GlobalSymbol, usize>) -> Self {
        GlobalSymbolSet {
            const_table,
            global_symbol_indexes: global_symbols,
        }
    }
}

pub struct RuntimeBuilder {
    global_context: GlobalContext,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        RuntimeBuilder {
            global_context: GlobalContext::new(),
        }
    }

    pub fn insert_global_symbols(&mut self, symbols: &GlobalSymbolSet) -> Result<(), RuntimeError> {
        let resolved_values = symbols.const_table.resolve(&self.global_context)?;
        let new_symbols: Result<Vec<_>, _> = symbols
            .global_symbol_indexes
            .iter()
            .map(|(symbol, index)| {
                Ok::<_, RuntimeError>((symbol.clone(), resolved_values.at(*index)?.clone()))
            })
            .collect();
        self.global_context.insert_symbols(new_symbols?)
    }

    pub fn build_with_main(self, main_symbol: &str) -> Result<Runtime, RuntimeError> {
        let main_function = self
            .global_context
            .lookup_symbol(&GlobalSymbol::new(main_symbol))
            .ok_or_else(|| RuntimeError::new_internal_error("Main symbol not found."))?;
        let initial_stack_frame = main_function
            .as_function()?
            .with(|f| f.make_stack_frame(Vec::new()))?;
        Ok(Runtime::new_with_initial_stack_frame(
            self.global_context,
            initial_stack_frame,
        ))
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
