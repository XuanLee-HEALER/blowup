//! Shared application state.
//!
//! `AppContext` is the canonical bundle of long-lived resources that
//! every adapter (Tauri desktop, axum server, future iOS bridge...)
//! wires up at startup: DB pool, library index, tracker manager,
//! torrent manager, shared HTTP client, event bus, task registry,
//! and the bearer token used to gate the HTTP API.
//!
//! Adapters share a single `Arc<AppContext>` so that service
//! functions in `blowup_core` can be called from either side with
//! the same state and there's exactly one place to extend when a
//! new shared resource is introduced.
//!
//! The Tauri adapter currently still registers each field as an
//! individual `State<T>` via `handle.manage(...)` to keep the
//! existing command signatures untouched, but it constructs them
//! via `AppContext::new(...)` so a missing or duplicated wiring is
//! a compile error in one place rather than drift across two
//! crates.

use crate::infra::events::EventBus;
use crate::library::index::LibraryIndex;
use crate::tasks::TaskRegistry;
use crate::torrent::manager::TorrentManager;
use crate::torrent::tracker::TrackerManager;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::OnceCell;

#[derive(Clone)]
pub struct AppContext {
    pub db: SqlitePool,
    pub library_index: Arc<LibraryIndex>,
    pub tracker: Arc<TrackerManager>,
    /// `OnceCell` because the torrent session is built asynchronously
    /// after the rest of the state is already up — handlers that need
    /// it go through `AppContext::torrent()` and get a 503-style error
    /// until it's ready.
    pub torrent: Arc<OnceCell<TorrentManager>>,
    pub http: reqwest::Client,
    pub events: EventBus,
    pub tasks: TaskRegistry,
    /// Bearer token required on every HTTP route. The Tauri adapter
    /// doesn't use this for its IPC commands (Tauri IPC is not
    /// cross-origin reachable), but it still owns a token value so
    /// the embedded HTTP server can gate LAN/iOS clients.
    pub auth_token: Arc<String>,
}

impl AppContext {
    #[allow(clippy::too_many_arguments)]
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

    /// Extract the torrent manager or return a "still initializing"
    /// message for handlers that need it before it's ready.
    pub fn torrent(&self) -> Result<&TorrentManager, String> {
        self.torrent
            .get()
            .ok_or_else(|| "torrent manager is still initializing".to_string())
    }
}
