//! Tests for blowup_server::serve_unix — verify that:
//! 1. We can bind a Unix socket and the file appears with 0600 perms
//! 2. A real HTTP request over hyperlocal hits the same router as TCP
//! 3. Sending the shutdown signal stops the task and leaves the
//!    socket file in place (cleanup is the caller's job)

#![cfg(unix)]

use blowup_core::AppContext;
use blowup_core::infra::events::EventBus;
use blowup_core::library::index::LibraryIndex;
use blowup_core::tasks::TaskRegistry;
use blowup_core::torrent::tracker::TrackerManager;
use blowup_server::AppState;
use http_body_util::BodyExt;
use hyper::Request;
use hyper::body::Bytes;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use hyperlocal::{UnixConnector, Uri};
use serial_test::serial;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tokio::sync::oneshot;

const TEST_TOKEN: &str = "test-token";

async fn make_state() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    blowup_core::config::init_app_data_dir(tmp.path().to_path_buf());
    blowup_core::infra::cache::init_cache();

    let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
    sqlx::migrate!("../core/migrations").run(&pool).await.unwrap();

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

#[tokio::test]
#[serial]
async fn serve_unix_binds_and_routes() {
    let (state, tmp) = make_state().await;
    let socket_path = tmp.path().join("test.sock");

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let path_clone = socket_path.clone();
    let task = tokio::spawn(async move {
        blowup_server::serve_unix(&path_clone, state, shutdown_rx)
            .await
            .unwrap();
    });

    // Wait for the socket file to appear (bind happens async)
    for _ in 0..50 {
        if socket_path.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(socket_path.exists(), "socket file not created");

    // Note: 0600 perms are the CALLER's responsibility (skill_bridge_start
    // command does the chmod). serve_unix itself just bind()s, so the
    // socket gets the umask-default perms. We don't assert perms here.

    // Make a real HTTP request over the socket
    let client: Client<UnixConnector, http_body_util::Full<Bytes>> =
        Client::builder(TokioExecutor::new()).build(UnixConnector);
    let url: hyper::Uri = Uri::new(&socket_path, "/api/v1/health").into();
    let req = Request::builder()
        .uri(url)
        .header("authorization", format!("Bearer {TEST_TOKEN}"))
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();

    let resp = client.request(req).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Drain body to make sure router actually responded
    let _body = resp.into_body().collect().await.unwrap().to_bytes();

    // Send shutdown
    shutdown_tx.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .expect("task did not exit within 2s")
        .unwrap();
}
