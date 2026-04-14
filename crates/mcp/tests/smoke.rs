//! Cross-process smoke test for blowup-mcp.
//!
//! Spawns the real bridge binary as a child process, mounts a minimal
//! axum router on a tempdir Unix socket, and sends JSON-RPC requests
//! through the bridge's stdin to verify the full chain
//! (stdio → rmcp → hyperlocal → axum) works end-to-end.

#![cfg(unix)]

use serde_json::{Value, json};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;

const BRIDGE_BIN: &str = env!("CARGO_BIN_EXE_blowup-mcp");

async fn start_test_router(socket_path: &std::path::Path) -> oneshot::Sender<()> {
    use axum::Router;
    use axum::routing::{get, put};

    let router: Router = Router::new()
        .route(
            "/api/v1/health",
            get(|| async { axum::Json(json!({ "ok": true })) }),
        )
        .route(
            "/api/v1/entries",
            get(|| async {
                axum::Json(json!([
                    {
                        "id": 1,
                        "name": "测试条目",
                        "tags": ["测试"],
                        "updated_at": "2026-04-14T00:00:00Z"
                    }
                ]))
            })
            .post(|| async { axum::Json(42_i64) }),
        )
        .route(
            "/api/v1/entries/tags",
            get(|| async { axum::Json(vec!["导演", "电影", "测试"]) }),
        )
        .route(
            "/api/v1/entries/relation-types",
            get(|| async { axum::Json(vec!["导演了", "主演了"]) }),
        )
        .route(
            "/api/v1/entries/{id}/wiki",
            put(|| async { axum::Json(serde_json::Value::Null) }),
        );

    let listener = tokio::net::UnixListener::bind(socket_path).unwrap();
    let (tx, rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    tx
}

async fn send_request(
    stdin: &mut tokio::process::ChildStdin,
    req: &Value,
) -> std::io::Result<()> {
    let line = serde_json::to_string(req).unwrap();
    stdin.write_all(line.as_bytes()).await?;
    stdin.write_all(b"\n").await?;
    stdin.flush().await
}

async fn read_response<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
    timeout_secs: u64,
) -> Value {
    let mut buf = String::new();
    let n = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        reader.read_line(&mut buf),
    )
    .await
    .expect("read_response timeout")
    .expect("read_response io error");
    assert!(n > 0, "EOF reading bridge response");
    serde_json::from_str(buf.trim()).expect("invalid JSON from bridge")
}

#[tokio::test(flavor = "multi_thread")]
async fn end_to_end_list_entries_through_bridge() {
    let tmp = tempfile::tempdir().unwrap();
    let socket_path = tmp.path().join("test.sock");
    let _shutdown = start_test_router(&socket_path).await;

    let mut child = Command::new(BRIDGE_BIN)
        .env("BLOWUP_MCP_SOCKET_OVERRIDE", &socket_path)
        .env("RUST_LOG", "blowup_mcp=warn")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn bridge");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // 1. initialize
    send_request(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "smoke", "version": "0" }
            }
        }),
    )
    .await
    .unwrap();
    let init_resp = read_response(&mut reader, 5).await;
    assert_eq!(init_resp["id"], 1);
    assert!(init_resp["error"].is_null(), "initialize error: {init_resp}");

    // 2. notifications/initialized — no response
    send_request(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
    )
    .await
    .unwrap();

    // 3. tools/list — verify all 9 tools are there
    send_request(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        }),
    )
    .await
    .unwrap();
    let list_resp = read_response(&mut reader, 5).await;
    let tools = list_resp["result"]["tools"]
        .as_array()
        .expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    for expected in &[
        "list_entries",
        "get_entry",
        "list_all_tags",
        "list_relation_types",
        "create_entry",
        "update_wiki",
        "update_name",
        "add_tag",
        "add_relation",
    ] {
        assert!(
            names.contains(expected),
            "tool {expected} missing from {names:?}"
        );
    }

    // 4. tools/call list_entries — read tool roundtrip
    send_request(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "list_entries",
                "arguments": { "query": null, "tag": null }
            }
        }),
    )
    .await
    .unwrap();
    let call_resp = read_response(&mut reader, 5).await;
    // rmcp may put errors at result.isError instead of jsonrpc-level error
    assert!(
        call_resp["error"].is_null(),
        "tools/call list_entries error: {call_resp}"
    );
    assert!(
        call_resp["result"]["isError"].as_bool() != Some(true),
        "tools/call list_entries returned isError: {call_resp}"
    );
    let content = &call_resp["result"]["content"];
    assert!(!content.is_null(), "content missing from tools/call result");

    // 5. tools/call create_entry — write tool roundtrip
    send_request(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "create_entry",
                "arguments": { "name": "新条目" }
            }
        }),
    )
    .await
    .unwrap();
    let create_resp = read_response(&mut reader, 5).await;
    assert!(
        create_resp["error"].is_null(),
        "tools/call create_entry error: {create_resp}"
    );
    assert!(
        create_resp["result"]["isError"].as_bool() != Some(true),
        "tools/call create_entry returned isError: {create_resp}"
    );

    let _ = child.kill().await;
}
