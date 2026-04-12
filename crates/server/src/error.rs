//! API error envelope.
//!
//! Services in `blowup_core` mostly return `Result<T, String>` today
//! (the domain-level enums in `blowup_core::error` are used by a few
//! modules). To keep this layer simple, we wrap any stringly error
//! from a service call into `ApiError::Internal` and map it to a
//! 500 response with a JSON `{ "error": "..." }` body.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            ApiError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        };
        (status, Json(ErrorBody { error: msg })).into_response()
    }
}

impl From<String> for ApiError {
    fn from(s: String) -> Self {
        // Heuristic mapping from core's string errors. Services use
        // Chinese error messages with characteristic phrases; we only
        // probe for a couple of common ones here.
        if s.contains("未找到") || s.contains("not found") {
            ApiError::NotFound(s)
        } else {
            ApiError::Internal(s)
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
