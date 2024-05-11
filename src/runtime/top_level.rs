use crate::{
    binary::modules::ModuleId,
    gc::{GcTraceable, PinnedGcRef},
};

use super::{
    error::Result,
    eval_context::EvalContext,
    global_env::GlobalEnv,
    stack_frame::{LocalStack, StackContext},
    value::Value,
};

pub struct Stack<'a> {
    stack_context: StackContext<'a>,
}

impl<'a> std::ops::Deref for Stack<'a> {
    type Target = StackContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.stack_context
    }
}

impl<'a> std::ops::DerefMut for Stack<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stack_context
    }
}

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
    inner: PinnedGcRef<Inner>,
}

impl TopLevelRuntime {
    pub(crate) fn new(global_context: GlobalEnv) -> Self {
        let inner = global_context.create_pinned_ref(Inner {
            stack: LocalStack::new(),
        });
        TopLevelRuntime {
            global_context,
            inner,
        }
    }

    pub fn stack(&self) -> Stack {
        Stack {
            stack_context: StackContext::new(
                &self.global_context.lock_collect(),
                &self.inner.stack,
            ),
        }
    }

    pub fn call_function(&self, num_args: u32) -> Result<u32> {
        let function = self.inner.stack.pop()?.as_function()?.pin();
        let mut eval_context = EvalContext::new(&self.global_context, &self.inner.stack);
        eval_context.run(function, num_args)
    }

    pub fn init_module(&self, module_id: &ModuleId) -> Result<()> {
        if let Some(init_func) = self.global_context.get_init_function(module_id)? {
            self.inner.stack.push(Value::Function(init_func));
            self.call_function(0)?;
            self.global_context.set_module_initialized(module_id)?;
        }
        Ok(())
    }
}
