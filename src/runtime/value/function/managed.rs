//! A managed function, representing code within the Loon runtime to evaluate.

use std::{cell::OnceCell, rc::Rc};

use crate::{
    gc::{GcRef, GcRefVisitor, GcTraceable, PinnedGcRef},
    runtime::{
        constants::ValueTable,
        global_env::GlobalEnv,
        instructions::InstEvalList,
        modules::ModuleGlobals,
        stack_frame::{LocalStack, PinnedValueBuffer, StackFrame},
        Result,
    },
};

/// A managed function, representing code within the Loon runtime to evaluate.
pub(crate) struct ManagedFunction {
    globals: GcRef<ModuleGlobals>,
    constants: OnceCell<GcRef<ValueTable>>,
    inst_list: Rc<InstEvalList>,
}

impl ManagedFunction {
    pub fn new_deferred(globals: PinnedGcRef<ModuleGlobals>, inst_list: Rc<InstEvalList>) -> Self {
        ManagedFunction {
            globals: globals.to_ref(),
            constants: OnceCell::new(),
            inst_list,
        }
    }

    pub fn make_stack_frame(
        &self,
        env: &GlobalEnv,
        args: &mut PinnedValueBuffer,
        local_stack: PinnedGcRef<LocalStack>,
    ) -> Result<PinnedGcRef<StackFrame>> {
        local_stack.push_iter(env, args.drain(..));
        Ok(StackFrame::new_managed(
            env,
            self.inst_list.clone(),
            self.constants().pin(),
            self.globals.pin(),
            local_stack,
        ))
    }

    pub fn constants(&self) -> &GcRef<ValueTable> {
        self.constants.get().expect("Constants not resolved.")
    }

    pub fn resolve_constants(&self, constants: PinnedGcRef<ValueTable>) {
        let result = self.constants.set(constants.to_ref());
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
