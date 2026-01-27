use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde::Serialize;
use std::fmt;

/// Main application error type with detailed categorization
#[derive(Debug)]
pub enum AppError {
    // Client errors (4xx)
    NotFound(String),
    InvalidInput(String),
    Conflict(String),
    Unauthorized(String),
    Forbidden(String),

    // Server errors (5xx)
    IoError(IoErrorContext),
    DatabaseError(DatabaseErrorContext),
    SerializationError(serde_json::Error),
    InternalError(String),

    // Specific domain errors
    VaultError(VaultError),
    FileSystemError(FileSystemError),
}

/// Context for IO errors with additional information
#[derive(Debug)]
pub struct IoErrorContext {
    pub error: std::io::Error,
    pub operation: String,
    pub path: Option<String>,
}

/// Context for database errors
#[derive(Debug)]
pub struct DatabaseErrorContext {
    pub error: sqlx::Error,
    pub operation: String,
    pub details: Option<String>,
}

/// Vault-specific errors
#[derive(Debug)]
pub enum VaultError {
    AlreadyExists(String),
    InvalidPath(String),
    NotAccessible(String),
    NotEmpty(String),
}

/// File system operation errors
#[derive(Debug)]
pub enum FileSystemError {
    PermissionDenied {
        path: String,
        operation: String,
    },
    PathTraversal {
        attempted_path: String,
    },
    FileTooBig {
        path: String,
        size: u64,
        max_size: u64,
    },
    InvalidFileName {
        name: String,
        reason: String,
    },
    DiskFull,
}

/// Structured error response for API
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_suggestion: Option<String>,
}

impl AppError {
    /// Get user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            AppError::NotFound(msg) => format!("The requested resource was not found: {}", msg),
            AppError::InvalidInput(msg) => format!("Invalid input: {}", msg),
            AppError::Conflict(msg) => format!("A conflict occurred: {}", msg),
            AppError::Unauthorized(msg) => format!("Authentication required: {}", msg),
            AppError::Forbidden(msg) => format!("Access denied: {}", msg),

            AppError::IoError(ctx) => {
                format!(
                    "File operation failed while {}: {}",
                    ctx.operation,
                    Self::friendly_io_message(&ctx.error)
                )
            }

            AppError::DatabaseError(ctx) => {
                format!(
                    "Database operation failed while {}: {}",
                    ctx.operation,
                    Self::friendly_db_message(&ctx.error)
                )
            }

            AppError::SerializationError(_) => {
                "Failed to process data format. The data may be corrupted.".to_string()
            }

            AppError::InternalError(msg) => {
                format!("An unexpected error occurred: {}", msg)
            }

            AppError::VaultError(err) => match err {
                VaultError::AlreadyExists(path) => {
                    format!("A vault already exists at this location: {}", path)
                }
                VaultError::InvalidPath(path) => {
                    format!("The vault path is invalid or inaccessible: {}", path)
                }
                VaultError::NotAccessible(path) => {
                    format!("Cannot access vault at: {}. Check permissions.", path)
                }
                VaultError::NotEmpty(path) => {
                    format!("The vault directory is not empty: {}", path)
                }
            },

            AppError::FileSystemError(err) => match err {
                FileSystemError::PermissionDenied { path, operation } => {
                    format!("Permission denied while {} file: {}", operation, path)
                }
                FileSystemError::PathTraversal { attempted_path } => {
                    format!("Invalid path (security violation): {}", attempted_path)
                }
                FileSystemError::FileTooBig {
                    path,
                    size,
                    max_size,
                } => {
                    format!(
                        "File too large: {} ({} bytes, max {} bytes)",
                        path, size, max_size
                    )
                }
                FileSystemError::InvalidFileName { name, reason } => {
                    format!("Invalid file name '{}': {}", name, reason)
                }
                FileSystemError::DiskFull => {
                    "Insufficient disk space to complete operation.".to_string()
                }
            },
        }
    }

    /// Get recovery suggestion for the error
    pub fn recovery_suggestion(&self) -> Option<String> {
        match self {
            AppError::NotFound(_) => {
                Some("Verify the resource exists and try again.".to_string())
            }
            AppError::Conflict(_) => {
                Some("Refresh and try again, or resolve the conflict manually.".to_string())
            }
            AppError::IoError(ctx) if ctx.error.kind() == std::io::ErrorKind::PermissionDenied => {
                Some("Check file permissions and ensure the application has access.".to_string())
            }
            AppError::DatabaseError(_) => {
                Some("Try restarting the application. If the problem persists, check database integrity.".to_string())
            }
            AppError::FileSystemError(FileSystemError::DiskFull) => {
                Some("Free up disk space and try again.".to_string())
            }
            AppError::FileSystemError(FileSystemError::PermissionDenied { .. }) => {
                Some("Check file permissions or run with appropriate privileges.".to_string())
            }
            AppError::VaultError(VaultError::InvalidPath(_)) => {
                Some("Ensure the path exists and is accessible.".to_string())
            }
            _ => None,
        }
    }

    /// Convert IO error to friendly message
    fn friendly_io_message(err: &std::io::Error) -> String {
        match err.kind() {
            std::io::ErrorKind::NotFound => "File or directory not found".to_string(),
            std::io::ErrorKind::PermissionDenied => "Permission denied".to_string(),
            std::io::ErrorKind::AlreadyExists => "File already exists".to_string(),
            std::io::ErrorKind::InvalidInput => "Invalid input".to_string(),
            std::io::ErrorKind::TimedOut => "Operation timed out".to_string(),
            std::io::ErrorKind::WriteZero => "Failed to write data".to_string(),
            std::io::ErrorKind::Interrupted => "Operation was interrupted".to_string(),
            std::io::ErrorKind::UnexpectedEof => "Unexpected end of file".to_string(),
            _ => format!("{}", err),
        }
    }

    /// Convert database error to friendly message
    fn friendly_db_message(err: &sqlx::Error) -> String {
        match err {
            sqlx::Error::RowNotFound => "Record not found in database".to_string(),
            sqlx::Error::ColumnNotFound(_) => "Database schema mismatch".to_string(),
            sqlx::Error::Database(db_err) => {
                if db_err.message().contains("UNIQUE") {
                    "A record with this value already exists".to_string()
                } else if db_err.message().contains("FOREIGN KEY") {
                    "Referenced record does not exist".to_string()
                } else {
                    format!("Database constraint violation: {}", db_err.message())
                }
            }
            _ => "Database operation failed".to_string(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.user_message())
    }
}

impl std::error::Error for AppError {}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        let status = match self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::IoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::SerializationError(_) => StatusCode::BAD_REQUEST,
            AppError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::VaultError(err) => match err {
                VaultError::AlreadyExists(_) => StatusCode::CONFLICT,
                VaultError::InvalidPath(_) => StatusCode::BAD_REQUEST,
                VaultError::NotAccessible(_) => StatusCode::FORBIDDEN,
                VaultError::NotEmpty(_) => StatusCode::CONFLICT,
            },
            AppError::FileSystemError(err) => match err {
                FileSystemError::PermissionDenied { .. } => StatusCode::FORBIDDEN,
                FileSystemError::PathTraversal { .. } => StatusCode::BAD_REQUEST,
                FileSystemError::FileTooBig { .. } => StatusCode::PAYLOAD_TOO_LARGE,
                FileSystemError::InvalidFileName { .. } => StatusCode::BAD_REQUEST,
                FileSystemError::DiskFull => StatusCode::INSUFFICIENT_STORAGE,
            },
        };

        let error_type = match self {
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::InvalidInput(_) => "INVALID_INPUT",
            AppError::Conflict(_) => "CONFLICT",
            AppError::Unauthorized(_) => "UNAUTHORIZED",
            AppError::Forbidden(_) => "FORBIDDEN",
            AppError::IoError(_) => "IO_ERROR",
            AppError::DatabaseError(_) => "DATABASE_ERROR",
            AppError::SerializationError(_) => "SERIALIZATION_ERROR",
            AppError::InternalError(_) => "INTERNAL_ERROR",
            AppError::VaultError(_) => "VAULT_ERROR",
            AppError::FileSystemError(_) => "FILESYSTEM_ERROR",
        };

        let response = ErrorResponse {
            error: error_type.to_string(),
            message: self.user_message(),
            details: self.to_string().into(),
            recovery_suggestion: self.recovery_suggestion(),
        };

        HttpResponse::build(status).json(response)
    }
}

// From implementations for automatic conversion
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::IoError(IoErrorContext {
            error: err,
            operation: "unknown operation".to_string(),
            path: None,
        })
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::DatabaseError(DatabaseErrorContext {
            error: err,
            operation: "unknown operation".to_string(),
            details: None,
        })
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::SerializationError(err)
    }
}

impl From<zip::result::ZipError> for AppError {
    fn from(err: zip::result::ZipError) -> Self {
        AppError::InternalError(format!("Zip operation failed: {}", err))
    }
}

// Helper constructors for better error context
impl AppError {
    pub fn io_error(
        operation: impl Into<String>,
        path: Option<String>,
        error: std::io::Error,
    ) -> Self {
        AppError::IoError(IoErrorContext {
            error,
            operation: operation.into(),
            path,
        })
    }

    pub fn db_error(operation: impl Into<String>, error: sqlx::Error) -> Self {
        AppError::DatabaseError(DatabaseErrorContext {
            error,
            operation: operation.into(),
            details: None,
        })
    }

    pub fn db_error_with_details(
        operation: impl Into<String>,
        error: sqlx::Error,
        details: impl Into<String>,
    ) -> Self {
        AppError::DatabaseError(DatabaseErrorContext {
            error,
            operation: operation.into(),
            details: Some(details.into()),
        })
    }
}

pub type AppResult<T> = Result<T, AppError>;
