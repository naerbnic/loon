use std::borrow::Cow;

pub struct TypeError {
    message: String,
}

pub struct ConversionError {
    message: String,
}

pub struct OperationPreconditionError {
    message: String,
}

pub enum RuntimeError {
    /// An error where the wrong type is used in an operation.
    Type(TypeError),
    Conversion(ConversionError),
    /// An error where an operation is attempted on an invalid state.
    OperationPrecondition(OperationPreconditionError),
}

impl RuntimeError {
    pub fn new_type_error<'a>(message: impl Into<Cow<'a, str>>) -> Self {
        Self::Type(TypeError {
            message: message.into().into_owned(),
        })
    }

    pub fn new_conversion_error<'a>(message: impl Into<Cow<'a, str>>) -> Self {
        Self::Conversion(ConversionError {
            message: message.into().into_owned(),
        })
    }

    pub fn new_operation_precondition_error<'a>(message: impl Into<Cow<'a, str>>) -> Self {
        Self::OperationPrecondition(OperationPreconditionError {
            message: message.into().into_owned(),
        })
    }
}
