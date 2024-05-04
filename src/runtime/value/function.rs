use std::rc::Rc;

use crate::{
    refs::{GcRef, GcRefVisitor, GcTraceable},
    runtime::{
        constants::ValueTable, error::RuntimeError, instructions::InstEvalList,
        modules::ModuleGlobals, stack_frame::{LocalStack, StackFrame}, value::Value,
    },
};

use self::managed::ManagedFunction;

pub mod managed;
pub mod native;

pub struct Closure {
    function: GcRef<Function>,
    captured_values: Vec<Value>,
}

pub(crate) enum Function {
    Managed(ManagedFunction),
    Closure(Closure),
}

impl Function {
    pub fn new_managed(
        global: ModuleGlobals,
        consts: ValueTable,
        inst_list: Rc<InstEvalList>,
    ) -> Self {
        Function::Managed(ManagedFunction::new(global, consts, inst_list))
    }

    pub fn new_closure(function: GcRef<Function>, captured_values: Vec<Value>) -> Self {
        Function::Closure(Closure {
            function,
            captured_values,
        })
    }

    pub fn make_stack_frame(
        &self,
        args: impl IntoIterator<Item = Value>,
        mut local_stack: LocalStack,
    ) -> Result<StackFrame, RuntimeError> {
        match self {
            Function::Managed(managed_func) => Ok(StackFrame::new(
                managed_func.inst_list().clone(),
                managed_func.constants().clone(),
                managed_func.globals().clone(),
                local_stack,
            )),
            Function::Closure(closure) => {
                local_stack.push_iter(closure.captured_values.iter().cloned());
                let args: Vec<_> = closure.captured_values.iter().cloned().chain(args).collect();
                let stack_frame = closure
                    .function
                    .try_with(move |f| f.make_stack_frame(args, local_stack))
                    .ok_or_else(|| {
                        RuntimeError::new_internal_error("Function is not available.")
                    })??;
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
            Function::Managed(_) => {}
            Function::Closure(closure) => {
                for value in closure.captured_values.iter() {
                    value.trace(visitor);
                }
                visitor.visit(&closure.function);
            }
        }
    }
}
