//! Standalone blowup-server entry point.
//!
//! Boots the core state (config dir, DB, library index, tracker
//! manager, torrent manager) from environment variables, then starts
//! the axum router on `$BLOWUP_SERVER_BIND` (default 127.0.0.1:17690).
//!
//! Environment variables:
//!   BLOWUP_DATA_DIR   — override app data directory (default: dirs::data_dir()/blowup-server)
//!   BLOWUP_SERVER_BIND — override bind address (default: 127.0.0.1:17690)

use blowup_core::config;
use blowup_core::infra::db;
use blowup_core::library::index::LibraryIndex;
use blowup_core::torrent::manager::TorrentManager;
use blowup_core::torrent::tracker::TrackerManager;
use blowup_server::{AppState, build_router};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::OnceCell;

const DEFAULT_BIND: &str = "127.0.0.1:17690";

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("blowup_server=info,blowup_core=info,tower_http=info"));
    fmt().with_env_filter(filter).init();
}

fn resolve_data_dir() -> PathBuf {
    std::env::var("BLOWUP_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("blowup-server")
        })
}

fn resolve_bind() -> String {
    std::env::var("BLOWUP_SERVER_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let data_dir = resolve_data_dir();
    std::fs::create_dir_all(&data_dir).ok();
    tracing::info!(path = %data_dir.display(), "using data dir");
    config::init_app_data_dir(data_dir.clone());
    blowup_core::infra::cache::init_cache();

    let cfg = config::load_config();

    // DB
    let pool = db::init_db(&data_dir)
        .await
        .map_err(|e| format!("db init failed: {e}"))?;
    tracing::info!("database ready");

    // Library index
    let root_dir = shellexpand::tilde(&cfg.library.root_dir).to_string();
    let library_root = PathBuf::from(&root_dir);
    std::fs::create_dir_all(&library_root).ok();
    let library_index = Arc::new(LibraryIndex::load(&library_root));
    tracing::info!(path = %library_root.display(), "library index loaded");

    // Tracker manager
    let (tracker_mgr, initial_trackers) = TrackerManager::load();
    let tracker = Arc::new(tracker_mgr);

    // Torrent manager — build it inline so the server is fully ready
    // before accepting requests. Standalone mode can afford the wait.
    let torrent_cell: Arc<OnceCell<TorrentManager>> = Arc::new(OnceCell::new());
    match TorrentManager::new(
        library_root,
        cfg.download.max_concurrent,
        cfg.download.enable_dht,
        cfg.download.persist_session,
        initial_trackers,
    )
    .await
    {
        Ok(tm) => {
            torrent_cell.set(tm).ok();
            tracing::info!("torrent manager ready");
        }
        Err(e) => {
            tracing::error!(error = %e, "torrent manager init failed — downloads routes will return 503");
        }
    }

    let state = AppState::new(pool, library_index, tracker, torrent_cell);
    let app = build_router(state);

    let bind = resolve_bind();
    let listener = TcpListener::bind(&bind).await?;
    tracing::info!("blowup-server listening on http://{bind}");
    axum::serve(listener, app).await?;
    Ok(())
}
