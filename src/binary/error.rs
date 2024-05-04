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
}

#[derive(thiserror::Error, Debug)]
pub enum ValidationError {
    #[error("Found an invalid constant index")]
    LocalIndexResolutionError,
}

pub type Result<T> = std::result::Result<T, BuilderError>;
