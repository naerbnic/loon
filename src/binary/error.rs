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
}

pub type Result<T> = std::result::Result<T, BuilderError>;
