use std::rc::Rc;

use crate::{
    gc::{GcRef, GcRefVisitor, GcTraceable},
    runtime::{
        constants::ValueTable,
        error::{Result, RuntimeError},
        global_env::GlobalEnvLock,
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

pub struct Closure {
    function: GcRef<Function>,
    captured_values: Vec<Value>,
}

impl GcTraceable for Closure {
    fn trace<V>(&self, visitor: &mut V)
    where
        V: GcRefVisitor,
    {
        visitor.visit(&self.function);
        for value in &self.captured_values {
            value.trace(visitor);
        }
    }
}

pub(crate) enum Function {
    Managed(ManagedFunction),
    Native(NativeFunctionPtr),
    Closure(Closure),
}

impl Function {
    pub fn new_managed_deferred(
        global_env: &GlobalEnvLock,
        global: ModuleGlobals,
        inst_list: Rc<InstEvalList>,
    ) -> (GcRef<Self>, impl FnOnce(ValueTable)) {
        let base_func_value = global_env.create_ref(Function::Managed(
            ManagedFunction::new_deferred(global, inst_list),
        ));

        (base_func_value.clone(), move |value_table| {
            let base_func = base_func_value.borrow();
            let Function::Managed(managed_func) = &*base_func else {
                unreachable!()
            };
            managed_func.resolve_constants(value_table);
        })
    }

    pub fn new_native<T>(global_env: &GlobalEnvLock, native_func: T) -> GcRef<Self>
    where
        T: native::NativeFunction + 'static,
    {
        global_env.create_ref(Function::Native(NativeFunctionPtr::new(native_func)))
    }

    pub fn new_closure(
        global_env: &GlobalEnvLock,
        function: GcRef<Function>,
        captured_values: Vec<Value>,
    ) -> GcRef<Self> {
        global_env.create_ref(Function::Closure(Closure {
            function,
            captured_values,
        }))
    }

    pub fn bind_front(
        &self,
        global_env: &GlobalEnvLock,
        self_ref: &GcRef<Function>,
        captured_values: impl Sequence<Value>,
    ) -> GcRef<Self> {
        match self {
            Function::Managed(_) | Function::Native(_) => {
                Function::new_closure(global_env, self_ref.clone(), captured_values.collect())
            }
            Function::Closure(closure) => {
                let mut new_captured_values = closure.captured_values.clone();
                captured_values.extend_into(&mut new_captured_values);
                Function::new_closure(global_env, closure.function.clone(), new_captured_values)
            }
        }
    }

    pub fn make_stack_frame(
        &self,
        env_lock: &GlobalEnvLock,
        args: impl Sequence<Value>,
    ) -> Result<StackFrame> {
        self.make_stack_frame_inner(env_lock, args, LocalStack::new())
    }

    fn make_stack_frame_inner(
        &self,
        env_lock: &GlobalEnvLock,
        args: impl Sequence<Value>,
        local_stack: LocalStack,
    ) -> Result<StackFrame> {
        match self {
            Function::Managed(managed) => managed.make_stack_frame(env_lock, args, local_stack),
            Function::Native(native) => native.make_stack_frame(env_lock, args, local_stack),
            Function::Closure(closure) => {
                local_stack.push_sequence(wrap_iter(closure.captured_values.iter().cloned()));
                let stack_frame = closure
                    .function
                    .try_borrow()
                    .ok_or_else(|| RuntimeError::new_internal_error("Function is not available."))?
                    .make_stack_frame_inner(env_lock, args, local_stack)?;
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
            Function::Managed(managed) => managed.trace(visitor),
            Function::Native(native) => native.trace(visitor),
            Function::Closure(closure) => closure.trace(visitor),
        }
    }
}
