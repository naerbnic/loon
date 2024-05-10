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
        global_env: &GlobalEnvLock,
        global: ModuleGlobals,
        inst_list: Rc<InstEvalList>,
    ) -> (Self, impl FnOnce(ValueTable)) {
        let base_func_value = global_env.create_ref(BaseFunction::Managed(
            ManagedFunction::new_deferred(global, inst_list),
        ));

        (
            Function::Base(base_func_value.clone()),
            move |value_table| {
                let base_func = base_func_value.borrow();
                let managed_func = match &*base_func {
                    BaseFunction::Managed(managed_func) => managed_func,
                    _ => unreachable!(),
                };
                managed_func.resolve_constants(value_table);
            },
        )
    }

    pub fn new_native<T>(global_env: &GlobalEnvLock, native_func: T) -> Self
    where
        T: native::NativeFunction + 'static,
    {
        Function::Base(
            global_env.create_ref(BaseFunction::Native(NativeFunctionPtr::new(native_func))),
        )
    }

    pub fn new_closure(
        global_env: &GlobalEnvLock,
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
        global_env: &GlobalEnvLock,
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

    pub fn ref_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Function::Base(lhs), Function::Base(rhs)) => GcRef::ref_eq(lhs, rhs),
            (Function::Closure(lhs), Function::Closure(rhs)) => GcRef::ref_eq(lhs, rhs),
            _ => false,
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
