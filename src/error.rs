use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    InvalidInput(String),
    Conflict(String),
    IoError(std::io::Error),
    DatabaseError(sqlx::Error),
    SerializationError(serde_json::Error),
    InternalError(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            AppError::Conflict(msg) => write!(f, "Conflict: {}", msg),
            AppError::IoError(err) => write!(f, "IO error: {}", err),
            AppError::DatabaseError(err) => write!(f, "Database error: {}", err),
            AppError::SerializationError(err) => write!(f, "Serialization error: {}", err),
            AppError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        let status = match self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::IoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::SerializationError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        HttpResponse::build(status).json(serde_json::json!({
            "error": self.to_string(),
        }))
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::IoError(err)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::DatabaseError(err)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::SerializationError(err)
    }
}

pub type AppResult<T> = Result<T, AppError>;
