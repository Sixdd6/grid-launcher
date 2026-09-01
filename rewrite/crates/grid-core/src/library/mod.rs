pub mod paths;
pub mod registry;

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error(transparent)]
    Romm(#[from] crate::romm::RommError),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Extract(String),
    #[error("Archive extracted but no ROM file was found")]
    NoLaunchFile,
    #[error("cancelled")]
    Cancelled,
    #[error("Set a library folder in settings before installing games")]
    LibraryPathUnset,
    #[error("registry: {0}")]
    Registry(String),
}
