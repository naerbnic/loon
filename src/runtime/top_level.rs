use std::rc::{Rc, Weak};

use crate::gc::GcTraceable;

use super::{
    error::Result,
    eval_context::EvalContext,
    global_env::GlobalEnv,
    stack_frame::{LocalStack, StackContext},
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
        let inner = Rc::new(Inner {
            stack: LocalStack::new(),
        });
        global_context.add_top_level_contents(TopLevelContents(Rc::downgrade(&inner)));
        TopLevelRuntime {
            global_context,
            inner,
        }
    }

    pub fn stack(&self) -> StackContext {
        StackContext::new(&self.global_context, &self.inner.stack)
    }

    pub fn call_function(&self, num_args: u32) -> Result<u32> {
        let function = self.inner.stack.pop()?;
        let mut eval_context = EvalContext::new(&self.global_context, &self.inner.stack);
        eval_context.run(function.as_function()?.clone(), num_args)
    }
}

impl Drop for TopLevelRuntime {
    fn drop(&mut self) {
        let contents = TopLevelContents(Rc::downgrade(&self.inner));
        self.global_context.remove_top_level_contents(contents);
    }
}

pub(super) struct TopLevelContents(Weak<Inner>);

impl TopLevelContents {
    pub fn get_ptr(&self) -> *const () {
        self.0.as_ptr() as *const ()
    }
}

impl GcTraceable for TopLevelContents {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: crate::gc::GcRefVisitor,
    {
        if let Some(inner) = self.0.upgrade() {
            inner.trace(visitor);
        }
    }
}
