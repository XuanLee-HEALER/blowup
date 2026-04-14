//! Tauri commands for the skill bridge feature.
//!
//! 5 commands total: status / start / stop / install_to_claude_code /
//! get_install_snippets. All operate on the Tauri-managed
//! `SkillBridgeState`. Subsequent tasks (T13-T16) add the rest.

use crate::skill_bridge::state::SkillBridgeState;
use serde::Serialize;

#[derive(Serialize)]
pub struct SkillBridgeStatus {
    pub running: bool,
    pub socket_path: Option<String>,
    /// `false` on Windows for now — see plan "out of scope".
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
        supported: cfg!(unix),
    })
}
