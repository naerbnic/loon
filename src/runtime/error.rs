use std::borrow::Cow;

#[derive(Debug, thiserror::Error)]
#[error("Type Error: {message}")]
pub struct TypeError {
    message: String,
}

#[derive(Debug, thiserror::Error)]
#[error("Conversion Error: {message}")]
pub struct ConversionError {
    message: String,
}

#[derive(Debug, thiserror::Error)]
#[error("Operation precondition error: {message}")]
pub struct OperationPreconditionError {
    message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// An error where the wrong type is used in an operation.
    #[error(transparent)]
    Type(#[from] TypeError),
    #[error(transparent)]
    Conversion(#[from] ConversionError),
    /// An error where an operation is attempted on an invalid state.
    #[error(transparent)]
    OperationPrecondition(OperationPreconditionError),
    #[error("Internal error: {0}")]
    InternalError(String),
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

    pub fn new_internal_error<'a>(message: impl Into<Cow<'a, str>>) -> Self {
        Self::InternalError(message.into().into_owned())
    }
}

pub type Result<T> = std::result::Result<T, RuntimeError>;
