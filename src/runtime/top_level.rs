use std::rc::Rc;

use crate::gc::GcTraceable;

use super::{
    error::Result,
    global_env::GlobalEnv,
    stack_frame::{LocalStack, StackContext},
    EvalContext,
};

struct Inner {
    stack: LocalStack,
}

impl GcTraceable for Inner {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: crate::gc::GcRefVisitor,
    {
        self.stack.trace(visitor);
    }
}

pub struct TopLevelRuntime {
    global_context: GlobalEnv,
    inner: Rc<Inner>,
}

impl TopLevelRuntime {
    pub fn new(global_context: GlobalEnv) -> Self {
        TopLevelRuntime {
            global_context,
            inner: Rc::new(Inner {
                stack: LocalStack::new(),
            }),
        }
    }

    pub fn stack(&self) -> StackContext {
        StackContext::new(&self.global_context, &self.inner.stack)
    }

    pub fn call_function(&mut self, num_args: u32) -> Result<u32> {
        let function = self.inner.stack.pop()?;
        let mut eval_context = EvalContext::new(&self.global_context, &self.inner.stack);
        eval_context.run(function.as_function()?.clone(), num_args)
    }
}
