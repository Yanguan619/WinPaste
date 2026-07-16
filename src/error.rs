use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Database(String),
    IO(String),
    Network(String),
    Internal(String),
    Validation(String),
    Encryption(String),
}

impl std::error::Error for AppError {}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Database(e) => write!(f, "Database error: {}", e),
            AppError::IO(e) => write!(f, "IO error: {}", e),
            AppError::Network(e) => write!(f, "Network error: {}", e),
            AppError::Internal(e) => write!(f, "Internal error: {}", e),
            AppError::Validation(e) => write!(f, "Validation error: {}", e),
            AppError::Encryption(e) => write!(f, "Encryption error: {}", e),
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::Database(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::IO(err.to_string())
    }
}

impl From<String> for AppError {
    fn from(err: String) -> Self {
        AppError::Internal(err)
    }
}

impl From<image::ImageError> for AppError {
    fn from(err: image::ImageError) -> Self {
        AppError::IO(err.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
