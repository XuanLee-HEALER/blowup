//! blowup-mcp — stdio MCP bridge to the blowup desktop app.
//!
//! IMPORTANT: All logging goes to stderr because stdout is the MCP
//! JSON-RPC channel. Mixing them corrupts every tool call — do NOT
//! add `println!` anywhere in this binary or in `blowup_mcp::*`.

use blowup_mcp::service::BlowupService;
use rmcp::ServiceExt;
use rmcp::transport::stdio;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Tracing → stderr (NEVER stdout). Env filter defaults to
    // `blowup_mcp=info` so we get service lifecycle logs without
    // drowning in rmcp's own debug spam; user can override via
    // `RUST_LOG=blowup_mcp=debug,rmcp=debug`.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("blowup_mcp=info")),
        )
        .init();

    tracing::info!("blowup-mcp starting");

    // rmcp 1.4: `ServiceExt::serve` takes any `IntoTransport`, and
    // `(tokio::io::Stdin, tokio::io::Stdout)` implements it via the
    // AsyncRead+AsyncWrite blanket. `stdio()` returns exactly that
    // tuple. The returned `RunningService` runs the request loop in
    // its own task; `.waiting()` blocks until the transport closes
    // (EOF on stdin).
    let service = BlowupService::new();
    let server = service.serve(stdio()).await?;
    let reason = server.waiting().await?;
    tracing::info!(?reason, "blowup-mcp exiting");
    Ok(())
}
