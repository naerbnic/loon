#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum BuilderError {
    #[error("Value already exists.")]
    AlreadyExists,

    #[error("Expected a moudle const.")]
    ExpectedModuleConst,

    #[error("Mismatched builder.")]
    MismatchedBuilder,

    #[error("Deferred value not resolved.")]
    DeferredNotResolved,

    #[error(transparent)]
    Validation(#[from] ValidationError),

    #[error("Reference was unresolved.")]
    UnresolvedReference,

    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl BuilderError {
    pub fn new_other<E>(error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        BuilderError::Other(Box::new(error))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ValidationError {
    #[error("Found an invalid constant index")]
    LocalIndexResolutionError,
}

pub type Result<T> = std::result::Result<T, BuilderError>;
