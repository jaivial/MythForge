//! Single error type crossing the API boundary, mapped to HTTP status codes.

use axum::http::StatusCode;
use axum::response::Response;
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    BadRequest(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("ai error: {0}")]
    Ai(String),
    #[error("{0}")]
    Internal(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<misanthropy::Error> for Error {
    fn from(e: misanthropy::Error) -> Self {
        Error::Ai(e.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for Error {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        Error::Unauthorized(e.to_string())
    }
}

impl From<argon2::password_hash::Error> for Error {
    fn from(e: argon2::password_hash::Error) -> Self {
        Error::Unauthorized(format!("password hash error: {e}"))
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Internal(format!("http error: {e}"))
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Internal(format!("json error: {e}"))
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Internal(e.to_string())
    }
}

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match &self {
            Error::BadRequest(_) => StatusCode::BAD_REQUEST,
            Error::NotFound(_) => StatusCode::NOT_FOUND,
            Error::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Error::Forbidden(_) => StatusCode::FORBIDDEN,
            Error::Conflict(_) => StatusCode::CONFLICT,
            Error::Db(sqlx::Error::RowNotFound) => StatusCode::NOT_FOUND,
            Error::Db(_) | Error::Ai(_) | Error::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let code = match &self {
            Error::BadRequest(_) => "bad_request",
            Error::NotFound(_) => "not_found",
            Error::Unauthorized(_) => "unauthorized",
            Error::Forbidden(_) => "forbidden",
            Error::Conflict(_) => "conflict",
            Error::Db(_) => "database_error",
            Error::Ai(_) => "ai_error",
            Error::Internal(_) => "internal_error",
        };
        let body = json!({ "error": code, "message": self.to_string() });
        (status, Json(body)).into_response()
    }
}
