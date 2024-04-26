use std::rc::Rc;

use crate::{
    refs::{GcRef, GcRefVisitor, GcTraceable},
    runtime::{
        error::RuntimeError, instructions::InstEvalList, stack_frame::StackFrame, value::Value,
    },
};

use self::managed::ManagedFunction;

pub mod managed;
pub mod native;

pub struct Closure {
    function: GcRef<Function>,
    captured_values: Vec<Value>,
}

pub enum Function {
    Managed(ManagedFunction),
    Closure(Closure),
}

impl Function {
    pub fn new_managed(consts: Vec<Value>, inst_list: Rc<InstEvalList>) -> Self {
        Function::Managed(ManagedFunction::new(Rc::new(consts), inst_list))
    }

    pub fn make_stack_frame(&self, args: Vec<Value>) -> Result<StackFrame, RuntimeError> {
        match self {
            Function::Managed(managed_func) => {
                Ok(StackFrame::new(managed_func.inst_list().clone(), args))
            }
            Function::Closure(closure) => {
                let mut inner_args = closure.captured_values.clone();
                inner_args.extend(args);
                let stack_frame = closure
                    .function
                    .try_with(move |f| f.make_stack_frame(inner_args))
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
