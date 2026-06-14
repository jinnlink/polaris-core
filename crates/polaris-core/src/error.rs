#[derive(Debug, thiserror::Error)]
pub enum PolarisError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("pack error: {0}")]
    Pack(#[from] crate::pack::PackError),
    #[error("missing attempt {0}")]
    MissingAttempt(String),
    #[error("missing concept {0}")]
    MissingConcept(String),
    #[error("missing goal {0}")]
    MissingGoal(String),
    #[error("invalid graph node {id}: expected {expected}")]
    InvalidGraphNode { id: String, expected: String },
    #[error("invalid grader response: {0}")]
    InvalidGraderResponse(String),
    #[error("invalid parameter {key}: {value}")]
    InvalidParameter { key: String, value: String },
}

pub type Result<T> = std::result::Result<T, PolarisError>;
