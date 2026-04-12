//! blowup-server: axum HTTP wrapper around `blowup_core`.
//!
//! Exposed both as a standalone binary (`main.rs`) and as a library
//! so `blowup-tauri` can spawn the router in-process for LAN-side
//! iPad access (see step 5 of docs/REFACTOR.md).

pub mod error;
pub mod routes;
pub mod state;

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub use state::AppState;

/// Build the full axum Router for blowup-server, mounted under /api/v1.
pub fn build_router(state: AppState) -> Router {
    let api = Router::new()
        .merge(routes::health::router())
        .merge(routes::config::router())
        .with_state(state);

    Router::new()
        .nest("/api/v1", api)
        .layer(CorsLayer::very_permissive())
        .layer(TraceLayer::new_for_http())
}
