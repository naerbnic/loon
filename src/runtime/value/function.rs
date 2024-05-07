use std::rc::Rc;

use crate::{
    gc::{GcRef, GcRefVisitor, GcTraceable},
    runtime::{
        constants::ValueTable,
        error::{Result, RuntimeError},
        global_env::GlobalEnv,
        instructions::InstEvalList,
        modules::ModuleGlobals,
        stack_frame::{LocalStack, StackFrame},
        value::Value,
    },
    util::sequence::{wrap_iter, Sequence},
};

use self::managed::ManagedFunction;
use self::native::NativeFunctionPtr;

pub mod managed;
pub mod native;

pub enum BaseFunction {
    Managed(ManagedFunction),
    Native(NativeFunctionPtr),
}

impl BaseFunction {
    pub fn make_stack_frame(
        &self,
        args: impl Sequence<Value>,
        local_stack: LocalStack,
    ) -> Result<StackFrame> {
        local_stack.push_sequence(args);
        match self {
            BaseFunction::Managed(managed_func) => Ok(StackFrame::new_managed(
                managed_func.inst_list().clone(),
                managed_func.constants().clone(),
                managed_func.globals().clone(),
                local_stack,
            )),
            BaseFunction::Native(native_func) => {
                Ok(StackFrame::new_native(native_func.clone(), local_stack))
            }
        }
    }
}

impl GcTraceable for BaseFunction {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        match self {
            BaseFunction::Managed(managed_func) => managed_func.trace(visitor),
            BaseFunction::Native(_) => {}
        }
    }
}

pub struct Closure {
    function: GcRef<BaseFunction>,
    captured_values: Vec<Value>,
}

impl GcTraceable for Closure {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        visitor.visit(&self.function);
        for value in self.captured_values.iter() {
            value.trace(visitor);
        }
    }
}

#[derive(Clone)]
pub(crate) enum Function {
    Base(GcRef<BaseFunction>),
    Closure(GcRef<Closure>),
}

impl Function {
    pub fn new_managed_deferred(
        global_env: &GlobalEnv,
        global: ModuleGlobals,
        inst_list: Rc<InstEvalList>,
    ) -> (Self, impl FnOnce(ValueTable)) {
        let (base_func_value, resolve_fn) = global_env.create_deferred_ref();

        (Function::Base(base_func_value), |value_table| {
            resolve_fn(BaseFunction::Managed(ManagedFunction::new(
                global,
                value_table,
                inst_list,
            )));
        })
    }

    pub fn new_native<T>(global_env: &GlobalEnv, native_func: T) -> Self
    where
        T: native::NativeFunction + 'static,
    {
        Function::Base(
            global_env.create_ref(BaseFunction::Native(NativeFunctionPtr::new(native_func))),
        )
    }

    pub fn new_closure(
        global_env: &GlobalEnv,
        function: GcRef<BaseFunction>,
        captured_values: Vec<Value>,
    ) -> Self {
        Function::Closure(global_env.create_ref(Closure {
            function,
            captured_values,
        }))
    }

    pub fn bind_front(
        &self,
        global_env: &GlobalEnv,
        captured_values: impl Sequence<Value>,
    ) -> Self {
        match self {
            Function::Base(base) => {
                Function::new_closure(global_env, base.clone(), captured_values.collect())
            }
            Function::Closure(closure) => {
                let closure = closure.borrow();
                let mut new_captured_values = closure.captured_values.clone();
                captured_values.extend_into(&mut new_captured_values);
                Function::new_closure(global_env, closure.function.clone(), new_captured_values)
            }
        }
    }

    pub fn make_stack_frame(&self, args: impl Sequence<Value>) -> Result<StackFrame> {
        self.make_stack_frame_inner(args, LocalStack::new())
    }

    fn make_stack_frame_inner(
        &self,
        args: impl Sequence<Value>,
        local_stack: LocalStack,
    ) -> Result<StackFrame> {
        match self {
            Function::Base(base) => base.borrow().make_stack_frame(args, local_stack),
            Function::Closure(closure) => {
                let closure = closure.borrow();
                local_stack.push_sequence(wrap_iter(closure.captured_values.iter().cloned()));
                let stack_frame = closure
                    .function
                    .try_borrow()
                    .ok_or_else(|| RuntimeError::new_internal_error("Function is not available."))?
                    .make_stack_frame(args, local_stack)?;
                Ok(stack_frame)
            }
        }
    }
}

impl GcTraceable for Function {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        match self {
            Function::Base(base) => visitor.visit(base),
            Function::Closure(closure) => visitor.visit(closure),
        }
    }
}
