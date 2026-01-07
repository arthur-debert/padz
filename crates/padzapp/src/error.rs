use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum PadzError {
    #[error("Pad not found: {0}")]
    PadNotFound(Uuid),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Store error: {0}")]
    Store(String),

    #[error("Api Error: {0}")]
    Api(String),
}

pub type Result<T> = std::result::Result<T, PadzError>;
