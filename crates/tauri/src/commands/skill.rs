//! Tauri commands for the skill bridge feature.
//!
//! 5 commands total: status / start / stop / install_to_claude_code /
//! get_install_snippets. All operate on the Tauri-managed
//! `SkillBridgeState`.

use crate::skill_bridge::state::SkillBridgeState;
use serde::Serialize;

/// Single source of truth for whether the skill bridge feature works
/// on the current build target. Today this is just `cfg!(unix)` —
/// Unix domain sockets and the hyperlocal stack don't support Windows
/// yet (Windows named pipes are out of scope for the MVP plan). Every
/// command in this file gates its writes on this constant so the
/// platform check stays in one place.
const SKILL_BRIDGE_SUPPORTED: bool = cfg!(unix);

#[derive(Serialize)]
pub struct SkillBridgeStatus {
    pub running: bool,
    pub socket_path: Option<String>,
    /// `false` on Windows for now — Settings UI shows "not yet
    /// supported" instead of a non-functional switch.
    pub supported: bool,
}

#[tauri::command]
pub async fn skill_bridge_status(
    state: tauri::State<'_, SkillBridgeState>,
) -> Result<SkillBridgeStatus, String> {
    Ok(SkillBridgeStatus {
        running: state.is_running(),
        socket_path: state
            .current_socket_path()
            .map(|p| p.to_string_lossy().into_owned()),
        supported: SKILL_BRIDGE_SUPPORTED,
    })
}
