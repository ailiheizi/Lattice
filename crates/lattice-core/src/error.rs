use thiserror::Error;

#[derive(Error, Debug)]
pub enum LatticeError {
    #[error("transport error: {0}")]
    Transport(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("search error: {0}")]
    Search(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("invalid signature")]
    InvalidSignature,

    #[error("untrusted sender: {0}")]
    UntrustedSender(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("already exists: {0}")]
    AlreadyExists(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, LatticeError>;
