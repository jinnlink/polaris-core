#[derive(Debug, thiserror::Error)]
pub enum PolarisError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("pack error: {0}")]
    Pack(#[from] crate::pack::PackError),
    #[error("missing attempt {0}")]
    MissingAttempt(String),
}

pub type Result<T> = std::result::Result<T, PolarisError>;
