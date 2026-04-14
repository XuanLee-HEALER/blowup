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

#[cfg(unix)]
use crate::skill_bridge::state::SkillBridgeHandle;
#[cfg(unix)]
use blowup_core::AppContext;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use tokio::sync::oneshot;

#[cfg(unix)]
#[tauri::command]
pub async fn skill_bridge_start(
    state: tauri::State<'_, SkillBridgeState>,
    ctx: tauri::State<'_, Arc<AppContext>>,
) -> Result<(), String> {
    if !SKILL_BRIDGE_SUPPORTED {
        return Err("Skill bridge 在 Windows 上暂未支持".to_string());
    }

    if state.is_running() {
        return Err("Skill bridge 已经在运行中".to_string());
    }

    let socket_path = blowup_mcp::socket::resolve_socket_path();
    ensure_parent_dir(&socket_path)?;
    handle_stale_socket(&socket_path).await?;

    // Bind via std first so we can chmod synchronously, then convert
    // to tokio's UnixListener.
    let std_listener = std::os::unix::net::UnixListener::bind(&socket_path)
        .map_err(|e| format!("bind {} 失败: {e}", socket_path.display()))?;
    std_listener
        .set_nonblocking(true)
        .map_err(|e| format!("set_nonblocking 失败: {e}"))?;
    let listener = tokio::net::UnixListener::from_std(std_listener)
        .map_err(|e| format!("from_std 失败: {e}"))?;

    // chmod 0600 — the socket file's permissions are the entire
    // security boundary for this feature.
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(&socket_path, perms)
        .map_err(|e| format!("chmod 0600 失败: {e}"))?;

    // Spawn the serve task. We can't call serve_unix(socket_path, ...)
    // here because we already bound the listener — passing the path
    // would try to bind a second time and fail. Instead we inline
    // axum::serve directly with the bound listener.
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let app_state: blowup_server::AppState = (**ctx).clone();
    let task = tokio::spawn(async move {
        let router = blowup_server::build_router(app_state);
        if let Err(e) = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
        {
            tracing::warn!(error = %e, "skill bridge serve exited");
        }
    });

    *state.0.lock() = Some(SkillBridgeHandle {
        task,
        shutdown_tx,
        socket_path,
    });
    Ok(())
}

/// Windows stub — same signature as the Unix version so the invoke
/// handler registration compiles unconditionally, but immediately
/// returns the "not supported" error.
#[cfg(not(unix))]
#[tauri::command]
pub async fn skill_bridge_start(
    _state: tauri::State<'_, SkillBridgeState>,
) -> Result<(), String> {
    Err("Skill bridge 在 Windows 上暂未支持".to_string())
}

#[cfg(unix)]
fn ensure_parent_dir(socket_path: &std::path::Path) -> Result<(), String> {
    let parent = socket_path
        .parent()
        .ok_or_else(|| "socket path has no parent".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("mkdir {} 失败: {e}", parent.display()))?;
    let perms = std::fs::Permissions::from_mode(0o700);
    let _ = std::fs::set_permissions(parent, perms);
    Ok(())
}

/// If the socket file already exists, try to connect to it. If we
/// can connect, another desktop instance is using it — bail. If we
/// can't, it's an orphan from a previous crash — unlink it.
#[cfg(unix)]
async fn handle_stale_socket(socket_path: &PathBuf) -> Result<(), String> {
    if !socket_path.exists() {
        return Ok(());
    }
    match tokio::net::UnixStream::connect(socket_path).await {
        Ok(_) => Err(format!("{} 已被另一个进程占用", socket_path.display())),
        Err(_) => {
            std::fs::remove_file(socket_path)
                .map_err(|e| format!("清理孤儿 socket 失败: {e}"))?;
            Ok(())
        }
    }
}
