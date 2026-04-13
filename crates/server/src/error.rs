//! API error envelope.
//!
//! Services in `blowup_core` mostly return `Result<T, String>`. To let
//! routes distinguish "not found" from "bad request" from "internal"
//! without scraping human-readable Chinese substrings, we rely on a
//! small set of well-known prefixes declared in
//! [`blowup_core::error::status`]:
//!
//!   "not_found: <msg>"   → HTTP 404
//!   "bad_request: <msg>" → HTTP 400
//!   anything else        → HTTP 500
//!
//! Core service functions that want a specific status code produce
//! their error string via `blowup_core::error::status::not_found(...)`
//! / `bad_request(...)`. Any string without a recognised prefix falls
//! through to 500, so callers that don't know/care still work.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use blowup_core::error::status::{BAD_REQUEST_PREFIX, NOT_FOUND_PREFIX};
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
        if let Some(msg) = s.strip_prefix(NOT_FOUND_PREFIX) {
            ApiError::NotFound(msg.to_string())
        } else if let Some(msg) = s.strip_prefix(BAD_REQUEST_PREFIX) {
            ApiError::BadRequest(msg.to_string())
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
