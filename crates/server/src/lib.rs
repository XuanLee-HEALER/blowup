//! blowup-server: axum HTTP wrapper around `blowup_core`.
//!
//! Exposed both as a standalone binary (`main.rs`) and as a library
//! so `blowup-tauri` can spawn the router in-process for LAN-side
//! iPad access (see step 5 of docs/REFACTOR.md).

pub mod auth;
pub mod error;
pub mod path_guard;
pub mod routes;
pub mod state;

use axum::Router;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

pub use state::AppState;

/// Bind + serve the axum router on `addr`. Convenience wrapper so
/// `blowup-tauri` can embed the server without adding a direct
/// `axum` dependency.
pub async fn serve(addr: &str, state: AppState) -> std::io::Result<()> {
    let router = build_router(state);
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, router)
        .await
        .map_err(std::io::Error::other)
}

/// Build the full axum Router for blowup-server, mounted under /api/v1.
///
/// Every route requires `Authorization: Bearer <token>` where the token
/// matches `state.auth_token`. CORS is intentionally *not* permissive:
/// no `Access-Control-Allow-Origin` header is emitted, so browsers will
/// block cross-origin preflights from random web pages even if they
/// somehow learn the token. Native clients (iOS, curl) are unaffected.
pub fn build_router(state: AppState) -> Router {
    let api = Router::new()
        .merge(routes::health::router())
        .merge(routes::config::router())
        .merge(routes::search::router())
        .merge(routes::tmdb::router())
        .merge(routes::media::router())
        .merge(routes::audio::router())
        .merge(routes::tracker::router())
        .merge(routes::subtitle::router())
        .merge(routes::entries::router())
        .merge(routes::library::router())
        .merge(routes::downloads::router())
        .merge(routes::export::router())
        .merge(routes::events::router())
        .merge(routes::tasks::router())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_bearer,
        ))
        .with_state(state);

    Router::new()
        .nest("/api/v1", api)
        .layer(TraceLayer::new_for_http())
}
