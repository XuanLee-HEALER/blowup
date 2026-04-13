//! Bearer-token auth middleware for blowup-server.
//!
//! Every route under `/api/v1` requires `Authorization: Bearer <token>`
//! where the token matches [`AppState::auth_token`]. The token is
//! resolved at startup from `$BLOWUP_SERVER_TOKEN` or generated
//! randomly (see `main.rs` / `build_router` callers).
//!
//! Why: the server exposes file-path-taking mutating routes
//! (`import_config`, `subtitle/shift`, `library/resources`, etc.)
//! that would otherwise be reachable from any page the user visits in
//! their browser (CORS doesn't fully save us, and doesn't save us at
//! all on a `0.0.0.0` LAN bind). See docs/REFACTOR.md step 5 for the
//! LAN/iOS deployment story.

use axum::{
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::Response,
};
use std::fmt::Write;

use crate::state::AppState;

/// Generate a fresh 48-character hex token (192 bits of entropy) for
/// use as a Bearer credential when the caller hasn't set
/// `$BLOWUP_SERVER_TOKEN`.
pub fn generate_random_token() -> String {
    let mut buf = [0u8; 24];
    getrandom::getrandom(&mut buf).expect("getrandom failed");
    let mut s = String::with_capacity(48);
    for b in &buf {
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

pub async fn require_bearer(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let Some(header) = req.headers().get(AUTHORIZATION) else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    let Ok(value) = header.to_str() else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    let Some(presented) = value.strip_prefix("Bearer ") else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    if constant_time_eq(presented.as_bytes(), state.auth_token.as_bytes()) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Length-leaking but value-constant-time byte comparison. Length of
/// the configured token is not secret, so the early-return on length
/// mismatch is fine here.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn equal_bytes_match() {
        assert!(constant_time_eq(b"abc", b"abc"));
    }

    #[test]
    fn different_bytes_differ() {
        assert!(!constant_time_eq(b"abc", b"abd"));
    }

    #[test]
    fn different_lengths_differ() {
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }
}
