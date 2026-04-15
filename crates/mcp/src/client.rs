//! Stateless HTTP client over a Unix domain socket.
//!
//! Every tool call instantiates one of these on demand — Unix socket
//! connect cost is ~zero so there's no point keeping a pool, and a
//! fresh connection means "open the switch" / "close the switch"
//! takes effect immediately.
//!
//! The client returns deserialized JSON or an `McpError` mapped from
//! the exact failure: connect refused → BridgeOffline; 4xx → BadRequest
//! / NotFound (with a hint provided by the caller); 5xx or body parse
//! → Internal.

use crate::error::McpError;
use crate::socket::resolve_socket_path;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::{Method, Request, StatusCode};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use hyperlocal::{UnixConnector, Uri as UnixUri};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::PathBuf;

pub struct BlowupClient {
    socket_path: PathBuf,
    inner: Client<UnixConnector, Full<Bytes>>,
}

impl BlowupClient {
    pub fn new() -> Self {
        Self {
            socket_path: resolve_socket_path(),
            inner: Client::builder(TokioExecutor::new()).build(UnixConnector),
        }
    }

    /// Optional hint passed by callers — appended to L3 error messages.
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        hint: Option<&str>,
    ) -> Result<T, McpError> {
        self.send::<(), T>(Method::GET, path, None, hint).await
    }

    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        hint: Option<&str>,
    ) -> Result<T, McpError> {
        self.send(Method::POST, path, Some(body), hint).await
    }

    pub async fn put<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        hint: Option<&str>,
    ) -> Result<T, McpError> {
        self.send(Method::PUT, path, Some(body), hint).await
    }

    async fn send<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
        hint: Option<&str>,
    ) -> Result<T, McpError> {
        let uri: hyper::Uri = UnixUri::new(&self.socket_path, path).into();
        let body_bytes = match body {
            Some(b) => serde_json::to_vec(b)
                .map_err(|e| McpError::internal(format!("serialize body: {e}")))?,
            None => Vec::new(),
        };

        let mut req = Request::builder().method(method).uri(uri);
        if !body_bytes.is_empty() {
            req = req.header("content-type", "application/json");
        }
        let req = req
            .body(Full::new(Bytes::from(body_bytes)))
            .map_err(|e| McpError::internal(format!("build request: {e}")))?;

        let resp = self.inner.request(req).await.map_err(|e| {
            // Underlying error is discarded from the user-visible
            // McpError because Claude shouldn't see "ECONNREFUSED" —
            // but log it at debug level so we can diagnose a flaky
            // socket without re-running with strace.
            tracing::debug!(error = %e, "blowup-mcp client connect failed");
            McpError::bridge_offline()
        })?;

        let status = resp.status();
        let body = resp
            .into_body()
            .collect()
            .await
            .map_err(|e| McpError::internal(format!("read body: {e}")))?
            .to_bytes();

        if status.is_success() {
            // Empty body → `null` → `()` deserializes cleanly, so
            // void writes (update_wiki, add_tag, …) can use the same
            // generic send() path as typed returns.
            if body.is_empty() {
                return serde_json::from_slice(b"null")
                    .map_err(|e| McpError::internal(format!("deserialize empty: {e}")));
            }
            return serde_json::from_slice(&body)
                .map_err(|e| McpError::internal(format!("deserialize: {e}")));
        }

        let text = String::from_utf8_lossy(&body).to_string();
        let hint_owned = hint.map(|h| h.to_string());
        Err(match status {
            StatusCode::NOT_FOUND => McpError::not_found(text, hint_owned),
            s if s.is_client_error() => McpError::bad_request(text, hint_owned),
            _ => McpError::internal(format!("HTTP {status}: {text}")),
        })
    }
}

impl Default for BlowupClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use serial_test::serial;

    /// U1: spin up an in-process axum server bound to a tempdir
    /// Unix socket and verify the client roundtrips JSON correctly.
    #[tokio::test]
    #[serial]
    async fn client_roundtrips_get_json() {
        use axum::Router;
        use axum::routing::get;
        use serde::Deserialize;
        use tokio::sync::oneshot;

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Echo {
            value: i32,
        }

        let tmp = tempfile::tempdir().unwrap();
        let socket_path = tmp.path().join("test.sock");
        unsafe {
            std::env::set_var("BLOWUP_MCP_SOCKET_OVERRIDE", &socket_path);
        }

        let router: Router =
            Router::new().route("/echo", get(|| async { axum::Json(Echo { value: 42 }) }));

        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });

        // Give the listener a beat to be ready
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = BlowupClient::new();
        let echo: Echo = client.get("/echo", None).await.unwrap();
        assert_eq!(echo, Echo { value: 42 });

        let _ = shutdown_tx.send(());
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), task).await;

        unsafe {
            std::env::remove_var("BLOWUP_MCP_SOCKET_OVERRIDE");
        }
    }

    #[tokio::test]
    #[serial]
    async fn client_returns_bridge_offline_when_socket_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let socket_path = tmp.path().join("nope.sock");
        unsafe {
            std::env::set_var("BLOWUP_MCP_SOCKET_OVERRIDE", &socket_path);
        }

        let client = BlowupClient::new();
        let result: Result<serde_json::Value, _> = client.get("/anything", None).await;
        let err = result.unwrap_err();
        assert_eq!(err.code(), ErrorCode::BridgeOffline);
        assert!(err.user_message().starts_with("[FATAL]"));

        unsafe {
            std::env::remove_var("BLOWUP_MCP_SOCKET_OVERRIDE");
        }
    }
}
