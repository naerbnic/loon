use std::borrow::Cow;

use crate::Value;

use super::{error::RuntimeError, Result};

pub(crate) struct LocalStack {
    stack: Vec<Value>,
}

impl LocalStack {
    pub fn new() -> Self {
        LocalStack { stack: Vec::new() }
    }

    pub fn from_args<'a>(args: impl Into<Cow<'a, [Value]>>) -> Self {
        LocalStack {
            stack: args.into().into_owned(),
        }
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    pub fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    pub fn pop(&mut self) -> Result<Value> {
        self.stack
            .pop()
            .ok_or_else(|| RuntimeError::new_operation_precondition_error("Local stack is empty."))
    }
}
