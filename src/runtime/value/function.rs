use std::rc::Rc;

use crate::{
    refs::{GcRef, GcRefVisitor, GcTraceable},
    runtime::{
        constants::ValueTable,
        context::GlobalEnv,
        error::{Result, RuntimeError},
        instructions::InstEvalList,
        modules::ModuleGlobals,
        stack_frame::{LocalStack, StackFrame},
        value::Value,
    },
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
        args: impl IntoIterator<Item = Value>,
        mut local_stack: LocalStack,
    ) -> Result<StackFrame> {
        local_stack.push_iter(args);
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
        captured_values: impl IntoIterator<Item = Value>,
    ) -> Self {
        match self {
            Function::Base(base) => Function::new_closure(
                global_env,
                base.clone(),
                captured_values.into_iter().collect(),
            ),
            Function::Closure(closure) => closure.with(|closure| {
                let mut new_captured_values = closure.captured_values.clone();
                new_captured_values.extend(captured_values);
                Function::new_closure(global_env, closure.function.clone(), new_captured_values)
            }),
        }
    }

    pub fn make_stack_frame(
        &self,
        args: impl IntoIterator<Item = Value>,
        mut local_stack: LocalStack,
    ) -> Result<StackFrame> {
        match self {
            Function::Base(base) => {
                base.with(|managed_func| managed_func.make_stack_frame(args, local_stack))
            }
            Function::Closure(closure) => closure.with(|closure| {
                local_stack.push_iter(closure.captured_values.iter().cloned());
                let args = closure.captured_values.iter().cloned().chain(args);
                let stack_frame = closure
                    .function
                    .try_with(move |f| f.make_stack_frame(args, local_stack))
                    .ok_or_else(|| {
                        RuntimeError::new_internal_error("Function is not available.")
                    })??;
                Ok(stack_frame)
            }),
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
