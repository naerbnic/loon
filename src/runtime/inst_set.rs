use super::{
    context::InstEvalContext,
    error::{Result, RuntimeError},
    instructions::{FunctionCallResult, InstEval, InstructionResult, InstructionTarget},
    stack_frame::LocalStack,
    value::Value,
};

#[derive(Clone, Debug)]
pub struct ReturnDynamic;

impl InstEval for ReturnDynamic {
    fn execute(
        &self,
        _ctxt: &InstEvalContext,
        _stack: &mut LocalStack,
    ) -> Result<InstructionResult> {
        Ok(InstructionResult::Return)
    }
}

#[derive(Clone, Debug)]
pub struct Add;

impl InstEval for Add {
    fn execute(
        &self,
        _ctxt: &InstEvalContext,
        stack: &mut LocalStack,
    ) -> Result<InstructionResult> {
        let a = stack.pop()?;
        let b = stack.pop()?;
        // Right now, only implement integer addition.
        let result = match (a, b) {
            (Value::Integer(i1), Value::Integer(i2)) => Value::Integer(i1.add_owned(i2)),
            (Value::Float(f1), Value::Float(f2)) => Value::Float(f1.add_owned(f2)),
            (_, _) => {
                return Err(RuntimeError::new_type_error(
                    "Addition is only supported for integers and floats.",
                ))
            }
        };
        stack.push(result);
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}

#[derive(Clone, Debug)]
pub struct CallDynamic;

impl InstEval for CallDynamic {
    fn execute(
        &self,
        _ctxt: &InstEvalContext,
        stack: &mut LocalStack,
    ) -> std::prelude::v1::Result<InstructionResult, super::error::RuntimeError> {
        let func = stack.pop()?.as_function()?.clone();
        let num_args = stack.pop()?.as_compact_integer()?;
        let num_args = u32::try_from(num_args).map_err(|_| {
            if num_args < 0 {
                super::error::RuntimeError::new_operation_precondition_error(
                    "Number of arguments is negative.",
                )
            } else {
                super::error::RuntimeError::new_operation_precondition_error(
                    "Number of arguments is too large.",
                )
            }
        })?;
        let mut args = Vec::with_capacity(usize::try_from(num_args).unwrap());
        for _ in 0..num_args {
            let arg = stack.pop()?;
            args.push(arg);
        }
        Ok(InstructionResult::Call(FunctionCallResult::new(
            func.clone(),
            args,
            InstructionTarget::Step,
        )))
    }
}

#[derive(Clone, Debug)]
pub struct PushConst(u32);

impl PushConst {
    pub fn new(index: u32) -> Self {
        PushConst(index)
    }
}

impl InstEval for PushConst {
    fn execute(&self, ctxt: &InstEvalContext, stack: &mut LocalStack) -> Result<InstructionResult> {
        let value = ctxt.get_constant(self.0)?;
        stack.push(value);
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}

#[derive(Clone, Debug)]
pub struct PushGlobal(u32);

impl PushGlobal {
    pub fn new(index: u32) -> Self {
        PushGlobal(index)
    }
}

impl InstEval for PushGlobal {
    fn execute(&self, ctxt: &InstEvalContext, stack: &mut LocalStack) -> Result<InstructionResult> {
        let value = ctxt.get_global(self.0)?;
        stack.push(value);
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}

#[derive(Clone, Debug)]
pub struct Pop(u32);

impl Pop {
    pub fn new(count: u32) -> Self {
        Pop(count)
    }
}

impl InstEval for Pop {
    fn execute(
        &self,
        _ctxt: &InstEvalContext,
        stack: &mut LocalStack,
    ) -> Result<InstructionResult> {
        for _ in 0..self.0 {
            stack.pop()?;
        }
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}

#[derive(Clone, Debug)]
pub struct SetGlobal(u32);

impl SetGlobal {
    pub fn new(index: u32) -> Self {
        SetGlobal(index)
    }
}

impl InstEval for SetGlobal {
    fn execute(&self, ctxt: &InstEvalContext, stack: &mut LocalStack) -> Result<InstructionResult> {
        let value = stack.pop()?;
        ctxt.set_global(self.0, value)?;
        Ok(InstructionResult::Next(InstructionTarget::Step))
    }
}
