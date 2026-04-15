//! Integration tests for `blowup_server::serve_unix`. Verifies:
//!   1. Bind a Unix socket, a real HTTP request through hyperlocal
//!      reaches the router, graceful-shutdown exits cleanly.
//!   2. The Unix socket path is **authless** — requests without an
//!      Authorization header still succeed. This is the skill-bridge
//!      security model (file perms do the access control, a bearer
//!      token would be redundant).
//!
//! Permissions and cleanup are the caller's job (the Tauri command
//! handles those), so nothing here touches perms.

#![cfg(unix)]

mod common;

use common::make_state;
use http_body_util::BodyExt;
use hyper::Request;
use hyper::body::Bytes;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use hyperlocal::{UnixConnector, Uri};
use serial_test::serial;
use tokio::sync::oneshot;

#[tokio::test]
#[serial]
async fn serve_unix_routes_without_auth_header() {
    let (state, tmp) = make_state().await;
    let socket_path = tmp.path().join("test.sock");

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let path_clone = socket_path.clone();
    let task = tokio::spawn(async move {
        blowup_server::serve_unix(&path_clone, state, shutdown_rx)
            .await
            .unwrap();
    });

    for _ in 0..50 {
        if socket_path.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(socket_path.exists(), "socket file not created");

    // No Authorization header — the Unix socket path MUST accept
    // unauthenticated requests because the skill bridge client
    // doesn't know the desktop's bearer token.
    let client: Client<UnixConnector, http_body_util::Full<Bytes>> =
        Client::builder(TokioExecutor::new()).build(UnixConnector);
    let url: hyper::Uri = Uri::new(&socket_path, "/api/v1/health").into();
    let req = Request::builder()
        .uri(url)
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();

    let resp = client.request(req).await.unwrap();
    assert_eq!(
        resp.status(),
        200,
        "health check should succeed without bearer"
    );
    let _body = resp.into_body().collect().await.unwrap().to_bytes();

    shutdown_tx.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .expect("task did not exit within 2s")
        .unwrap();
}
