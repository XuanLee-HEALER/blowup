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
}
