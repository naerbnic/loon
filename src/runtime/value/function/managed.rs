//! A managed function, representing code within the Loon runtime to evaluate.

use std::{cell::OnceCell, rc::Rc};

use crate::{
    gc::{GcRefVisitor, GcTraceable},
    runtime::{
        constants::ValueTable,
        global_env::GlobalEnvLock,
        instructions::InstEvalList,
        modules::ModuleGlobals,
        stack_frame::{LocalStack, StackFrame},
        value::Value,
        Result,
    },
    util::sequence::Sequence,
};

/// A managed function, representing code within the Loon runtime to evaluate.
pub(crate) struct ManagedFunction {
    globals: ModuleGlobals,
    constants: OnceCell<ValueTable>,
    inst_list: Rc<InstEvalList>,
}

impl ManagedFunction {
    pub fn new_deferred(globals: ModuleGlobals, inst_list: Rc<InstEvalList>) -> Self {
        ManagedFunction {
            globals,
            constants: OnceCell::new(),
            inst_list,
        }
    }

    pub fn make_stack_frame(
        &self,
        env_lock: &GlobalEnvLock,
        args: impl Sequence<Value>,
        local_stack: LocalStack,
    ) -> Result<StackFrame> {
        local_stack.push_sequence(args);
        dbg!(local_stack.len());
        Ok(StackFrame::new_managed(
            env_lock,
            self.inst_list.clone(),
            self.constants().clone(),
            self.globals.clone(),
            local_stack,
        ))
    }

    pub fn constants(&self) -> &ValueTable {
        self.constants.get().expect("Constants not resolved.")
    }

    pub fn resolve_constants(&self, constants: ValueTable) {
        let result = self.constants.set(constants);
        assert!(result.is_ok(), "Constants already resolved.");
    }
}

impl GcTraceable for ManagedFunction {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        self.globals.trace(visitor);
        if let Some(constants) = self.constants.get() {
            constants.trace(visitor);
        }
    }
}
