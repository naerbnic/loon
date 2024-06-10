use std::rc::Rc;

use crate::{
    gc::{GcRef, GcRefVisitor, GcTraceable, PinnedGcRef},
    runtime::{
        constants::ValueTable,
        error::{Result, RuntimeError},
        global_env::GlobalEnv,
        instructions::InstEvalList,
        modules::ModuleGlobals,
        stack_frame::{LocalStack, PinnedValueBuffer, StackFrame},
        value::Value,
    },
};

use self::managed::ManagedFunction;
use self::native::NativeFunctionPtr;

use super::PinnedValue;

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
        global_env: &GlobalEnv,
        global: PinnedGcRef<ModuleGlobals>,
        inst_list: Rc<InstEvalList>,
    ) -> (PinnedGcRef<Self>, impl FnOnce(PinnedGcRef<ValueTable>)) {
        let base_func_value = global_env.create_pinned_ref(Function::Managed(
            ManagedFunction::new_deferred(global, inst_list),
        ));

        (base_func_value.clone(), move |value_table| {
            let Function::Managed(managed_func) = &*base_func_value else {
                unreachable!()
            };
            managed_func.resolve_constants(value_table);
        })
    }

    pub fn new_native<T>(global_env: &GlobalEnv, native_func: T) -> PinnedGcRef<Self>
    where
        T: native::NativeFunction + 'static,
    {
        global_env.create_pinned_ref(Function::Native(NativeFunctionPtr::new(native_func)))
    }

    pub fn new_closure(
        global_env: &GlobalEnv,
        function: PinnedGcRef<Function>,
        captured_values: impl Iterator<Item = PinnedValue>,
    ) -> PinnedGcRef<Self> {
        global_env.with_lock(|lock| {
            global_env.create_pinned_ref(Function::Closure(Closure {
                function: function.into_ref(lock.guard()),
                captured_values: captured_values.map(|v| v.into_value(lock)).collect(),
            }))
        })
    }

    pub fn bind_front(
        &self,
        global_env: &GlobalEnv,
        self_ref: &PinnedGcRef<Function>,
        captured_values: &mut PinnedValueBuffer,
    ) -> PinnedGcRef<Self> {
        match self {
            Function::Managed(_) | Function::Native(_) => {
                Function::new_closure(global_env, self_ref.clone(), captured_values.drain(..))
            }
            Function::Closure(closure) => Function::new_closure(
                global_env,
                closure.function.pin(),
                closure
                    .captured_values
                    .iter()
                    .map(Value::pin)
                    .chain(captured_values.drain(..)),
            ),
        }
    }

    pub fn make_stack_frame(
        &self,
        env: &GlobalEnv,
        args: &mut PinnedValueBuffer,
    ) -> Result<PinnedGcRef<StackFrame>> {
        self.make_stack_frame_inner(env, args, LocalStack::new(env))
    }

    fn make_stack_frame_inner(
        &self,
        env: &GlobalEnv,
        args: &mut PinnedValueBuffer,
        local_stack: PinnedGcRef<LocalStack>,
    ) -> Result<PinnedGcRef<StackFrame>> {
        match self {
            Function::Managed(managed) => managed.make_stack_frame(env, args, local_stack),
            Function::Native(native) => native.make_stack_frame(env, args, local_stack),
            Function::Closure(closure) => {
                local_stack.push_iter(env, closure.captured_values.iter().map(Value::pin));
                let stack_frame = closure
                    .function
                    .try_borrow()
                    .ok_or_else(|| RuntimeError::new_internal_error("Function is not available."))?
                    .make_stack_frame_inner(env, args, local_stack)?;
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
