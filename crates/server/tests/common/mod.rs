//! Shared test harness for `crates/server/tests/`.
//!
//! Both `smoke.rs` and `serve_unix.rs` need the same in-memory
//! `AppState` — tempdir-backed SQLite, fresh library index, stub
//! tracker, a fixed bearer token. Keep the construction in one place
//! so every test sees the exact same setup.

#![allow(dead_code)] // each test binary uses a subset of the helpers

use blowup_core::AppContext;
use blowup_core::infra::events::EventBus;
use blowup_core::library::index::LibraryIndex;
use blowup_core::tasks::TaskRegistry;
use blowup_core::torrent::tracker::TrackerManager;
use blowup_server::AppState;
use std::sync::Arc;
use tokio::sync::OnceCell;

pub const TEST_TOKEN: &str = "test-token";

pub async fn make_state() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    blowup_core::config::init_app_data_dir(tmp.path().to_path_buf());
    blowup_core::infra::cache::init_cache();

    let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
    sqlx::migrate!("../core/migrations")
        .run(&pool)
        .await
        .unwrap();

    let library_root = tmp.path().join("library");
    std::fs::create_dir_all(&library_root).unwrap();
    let library_index = Arc::new(LibraryIndex::load(&library_root));

    let (tracker, _) = TrackerManager::load();
    let torrent = Arc::new(OnceCell::new());

    let state: AppState = AppContext::new(
        pool,
        library_index,
        Arc::new(tracker),
        torrent,
        EventBus::new(),
        TaskRegistry::new(),
        Arc::new(TEST_TOKEN.to_string()),
    );
    (state, tmp)
}
