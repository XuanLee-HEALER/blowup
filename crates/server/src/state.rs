//! Shared application state injected into every axum handler.
//!
//! Holds Arc-wrapped references to the same resources Tauri's setup
//! initializes: DB pool, library index, tracker manager, torrent
//! manager (lazy — OnceCell because the torrent session is built
//! asynchronously after startup). Cloneable so axum's `with_state`
//! can hand it to each request.

use blowup_core::infra::events::EventBus;
use blowup_core::library::index::LibraryIndex;
use blowup_core::tasks::TaskRegistry;
use blowup_core::torrent::manager::TorrentManager;
use blowup_core::torrent::tracker::TrackerManager;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::OnceCell;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub library_index: Arc<LibraryIndex>,
    pub tracker: Arc<TrackerManager>,
    pub torrent: Arc<OnceCell<TorrentManager>>,
    pub http: reqwest::Client,
    pub events: EventBus,
    pub tasks: TaskRegistry,
    /// Bearer token required by every route. Compared against the
    /// client-supplied `Authorization: Bearer <token>` header.
    pub auth_token: Arc<String>,
}

impl AppState {
    pub fn new(
        db: SqlitePool,
        library_index: Arc<LibraryIndex>,
        tracker: Arc<TrackerManager>,
        torrent: Arc<OnceCell<TorrentManager>>,
        events: EventBus,
        tasks: TaskRegistry,
        auth_token: Arc<String>,
    ) -> Self {
        Self {
            db,
            library_index,
            tracker,
            torrent,
            http: reqwest::Client::new(),
            events,
            tasks,
            auth_token,
        }
    }

    /// Extract the torrent manager or return a 503-style error message
    /// for handlers that need it before it's initialized.
    pub fn torrent(&self) -> Result<&TorrentManager, String> {
        self.torrent
            .get()
            .ok_or_else(|| "torrent manager is still initializing".to_string())
    }
}
