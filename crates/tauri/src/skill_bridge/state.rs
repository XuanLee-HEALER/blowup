//! Tauri-side runtime state for the skill bridge.
//!
//! Held inside a `Mutex<Option<SkillBridgeHandle>>` and managed by
//! the Tauri app handle. Created when the Settings switch turns ON,
//! taken+dropped when it turns OFF or the app exits.

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

pub struct SkillBridgeHandle {
    pub task: JoinHandle<()>,
    pub shutdown_tx: oneshot::Sender<()>,
    pub socket_path: PathBuf,
}

#[derive(Clone, Default)]
pub struct SkillBridgeState(pub Arc<Mutex<Option<SkillBridgeHandle>>>);

impl SkillBridgeState {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    pub fn is_running(&self) -> bool {
        self.0.lock().is_some()
    }

    pub fn current_socket_path(&self) -> Option<PathBuf> {
        self.0.lock().as_ref().map(|h| h.socket_path.clone())
    }

    /// Sync best-effort cleanup, safe to call from a window-close event
    /// handler or signal handler. Sends the shutdown signal and unlinks
    /// the socket file synchronously. Does NOT await the serve task —
    /// the Tokio runtime will reap it when the process exits.
    pub fn shutdown_blocking(&self) {
        if let Some(h) = self.0.lock().take() {
            let _ = h.shutdown_tx.send(());
            #[cfg(unix)]
            let _ = std::fs::remove_file(&h.socket_path);
            tracing::info!(
                path = %h.socket_path.display(),
                "skill bridge stopped via window close hook"
            );
        }
    }
}
