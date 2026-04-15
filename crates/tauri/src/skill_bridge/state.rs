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
pub struct SkillBridgeState(Arc<Mutex<Option<SkillBridgeHandle>>>);

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

    /// Install a fresh handle. Returns `Err` with the handle back if
    /// one is already present so the caller can decide (usually: don't
    /// overwrite — start() already rejects when `is_running`).
    pub fn install(&self, handle: SkillBridgeHandle) -> Result<(), SkillBridgeHandle> {
        let mut slot = self.0.lock();
        if slot.is_some() {
            return Err(handle);
        }
        *slot = Some(handle);
        Ok(())
    }

    /// Take the handle (if any) and signal shutdown + unlink the
    /// socket file. Returns the `JoinHandle` so async callers can
    /// `await` it under a timeout; sync callers (window close hooks)
    /// drop the return value and let the tokio runtime reap it.
    pub fn take_and_shutdown(&self) -> Option<JoinHandle<()>> {
        let handle = self.0.lock().take()?;
        let SkillBridgeHandle {
            task,
            shutdown_tx,
            socket_path,
        } = handle;
        let _ = shutdown_tx.send(());
        #[cfg(unix)]
        let _ = std::fs::remove_file(&socket_path);
        tracing::info!(
            path = %socket_path.display(),
            "skill bridge stopped"
        );
        Some(task)
    }

    /// Convenience for sync cleanup paths (window-close hook). Sends
    /// the shutdown signal and unlinks the socket file; does not
    /// await the serve task (the tokio runtime reaps it on exit).
    pub fn shutdown_blocking(&self) {
        let _ = self.take_and_shutdown();
    }
}
