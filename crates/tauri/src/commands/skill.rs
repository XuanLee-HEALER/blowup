//! Tauri commands for the skill bridge feature. Operate on the
//! Tauri-managed `SkillBridgeState`.

use crate::skill_bridge::state::SkillBridgeState;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Whether the skill bridge feature works on the current build
/// target. Unix domain sockets + hyperlocal don't support Windows
/// yet (Windows named pipes are out of scope for the MVP). Commands
/// gate their writes on this so the platform check lives in one place.
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
use std::sync::Arc;
#[cfg(unix)]
use tokio::sync::oneshot;

#[cfg(unix)]
#[tauri::command]
pub async fn skill_bridge_start(
    state: tauri::State<'_, SkillBridgeState>,
    ctx: tauri::State<'_, Arc<AppContext>>,
) -> Result<(), String> {
    if state.is_running() {
        return Err("Skill bridge 已经在运行中".to_string());
    }

    let socket_path = blowup_mcp::socket::resolve_socket_path();
    ensure_parent_dir(&socket_path)?;
    handle_stale_socket(&socket_path).await?;

    // Bind via `std` first so the chmod can happen synchronously
    // between bind and accept — `tokio::net::UnixListener::bind`
    // doesn't expose the same ordering.
    let std_listener = std::os::unix::net::UnixListener::bind(&socket_path)
        .map_err(|e| format!("bind {} 失败: {e}", socket_path.display()))?;
    std_listener
        .set_nonblocking(true)
        .map_err(|e| format!("set_nonblocking 失败: {e}"))?;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(&socket_path, perms)
        .map_err(|e| format!("chmod 0600 失败: {e}"))?;
    let listener = tokio::net::UnixListener::from_std(std_listener)
        .map_err(|e| format!("from_std 失败: {e}"))?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let app_state: blowup_server::AppState = (**ctx).clone();
    let task = tokio::spawn(async move {
        if let Err(e) =
            blowup_server::serve_unix_from_listener(listener, app_state, shutdown_rx).await
        {
            tracing::warn!(error = %e, "skill bridge serve exited");
        }
    });

    state
        .install(SkillBridgeHandle {
            task,
            shutdown_tx,
            socket_path,
        })
        .map_err(|_| "Skill bridge 已经在运行中".to_string())?;
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

/// Stop the skill bridge server. Idempotent — no-op if already
/// stopped. Waits up to 5 s for the serve task to drain in-flight
/// requests before returning so the caller knows the shutdown is
/// complete.
#[tauri::command]
pub async fn skill_bridge_stop(
    state: tauri::State<'_, SkillBridgeState>,
) -> Result<(), String> {
    if let Some(task) = state.take_and_shutdown() {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), task).await;
    }
    Ok(())
}

#[cfg(unix)]
fn ensure_parent_dir(socket_path: &std::path::Path) -> Result<(), String> {
    let parent = socket_path
        .parent()
        .ok_or_else(|| "socket path has no parent".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("mkdir {} 失败: {e}", parent.display()))?;
    // Best-effort hardening: 0700 on the parent dir keeps other
    // local users from probing the socket. Failing here doesn't
    // block startup (the socket file's own 0600 perms below are
    // the actual security boundary), but log it so an operator can
    // notice a degraded state.
    let perms = std::fs::Permissions::from_mode(0o700);
    if let Err(e) = std::fs::set_permissions(parent, perms) {
        tracing::warn!(
            error = %e,
            path = %parent.display(),
            "failed to chmod 0700 skill bridge parent dir; relying on socket 0600"
        );
    }
    Ok(())
}

/// Try to connect to the socket at `socket_path`. If the connect
/// succeeds another desktop is using it — bail. Otherwise assume an
/// orphan from a previous crash and unlink. A `NotFound` unlink is
/// the common no-prior-run case.
///
/// Known limitation: `connect` can also fail under load if the
/// owning process's accept backlog is momentarily full, in which
/// case we'd unlink a live socket. Acceptable for a single-user
/// desktop tool. If this ever bites, switch to `getpeercred` on a
/// successful connect.
#[cfg(unix)]
async fn handle_stale_socket(socket_path: &std::path::Path) -> Result<(), String> {
    match tokio::net::UnixStream::connect(socket_path).await {
        Ok(_) => Err(format!("{} 已被另一个进程占用", socket_path.display())),
        Err(_) => match std::fs::remove_file(socket_path) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("清理孤儿 socket 失败: {e}")),
        },
    }
}

#[derive(Serialize)]
pub struct InstallSnippets {
    pub binary_path: String,
    /// Shell command for Claude Code users: `claude mcp add …`.
    pub claude_code: String,
    /// `mcpServers` JSON block accepted by Claude Desktop, Cursor,
    /// Cline, Zed, and any other client that speaks the standard
    /// MCP config schema. Same bytes for all of them.
    pub mcp_servers_json: String,
}

#[tauri::command]
pub async fn skill_bridge_get_install_snippets(
    app: tauri::AppHandle,
) -> Result<InstallSnippets, String> {
    let bin = installed_binary_path(&app)?;
    let bin_str = bin.to_string_lossy().to_string();

    let claude_code = format!(
        "claude mcp add --scope user blowup-skill {}",
        shell_escape(&bin_str)
    );

    let mcp_servers_json = serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": {
            "blowup-skill": { "command": &bin_str, "args": [] }
        }
    }))
    .map_err(|e| format!("serialize snippet: {e}"))?;

    Ok(InstallSnippets {
        binary_path: bin_str,
        claude_code,
        mcp_servers_json,
    })
}

/// The path where `skill_bridge_install_to_claude_code` will copy the
/// binary. Same function used by both commands so the snippets and the
/// install action agree on the target.
///
/// Resolved via Tauri's `local_data_dir` so the location is
/// platform-correct out of the box: on macOS it returns
/// `~/Library/Application Support/blowup/blowup-mcp` (the Apple
/// convention, visible in Finder), and on Linux it returns
/// `~/.local/share/blowup/blowup-mcp` (XDG).
fn installed_binary_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .local_data_dir()
        .map_err(|e| format!("local_data_dir: {e}"))?;
    Ok(dir.join("blowup").join("blowup-mcp"))
}

/// Wrap a path in a single-quoted POSIX shell literal, escaping any
/// embedded single quotes the canonical way (`'\''`). Returns the
/// input unchanged if every character is in the safe set
/// (`a-z A-Z 0-9 / - . _`).
///
/// Known edge cases (acceptable for the install snippet use case):
/// - Newlines in the path break out of the single-quote span. Don't
///   put your installer in a directory whose name has a newline.
/// - In an interactive bash shell, `!` triggers history expansion
///   even inside single quotes. Paths containing `!` will produce a
///   `bash: event not found` error when the user pastes the snippet.
fn shell_escape(s: &str) -> String {
    if s.chars().all(|c| c.is_alphanumeric() || "/-._".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[derive(Serialize)]
pub struct InstallReport {
    pub binary_path: String,
    pub skill_path: String,
    pub claude_added: bool,
    pub manual_command: Option<String>,
}

#[tauri::command]
pub async fn skill_bridge_install_to_claude_code(
    app: tauri::AppHandle,
) -> Result<InstallReport, String> {
    use tauri::Manager;

    if !SKILL_BRIDGE_SUPPORTED {
        return Err("Skill bridge 在 Windows 上暂未支持".to_string());
    }

    // Paths resolved on the async side — no disk IO yet.
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("resource_dir: {e}"))?;
    let target_binary = installed_binary_path(&app)?;
    // Validate UTF-8 up front so the Command::args below doesn't
    // silently pass an empty string on non-UTF-8 paths.
    let target_binary_str = target_binary
        .to_str()
        .ok_or_else(|| {
            format!(
                "install path is not valid UTF-8: {}",
                target_binary.display()
            )
        })?
        .to_string();

    // File copies, SHA hashes, mkdirs, chmod, and the `claude mcp
    // add` subprocess all block — run the whole sequence on a
    // blocking worker so the Tauri async runtime stays responsive.
    let report = tokio::task::spawn_blocking(move || -> Result<InstallReport, String> {
        let bundled_binary = resource_dir.join("blowup-mcp");
        let bundled_skill = resource_dir
            .join("skills")
            .join("blowup-wiki-writer")
            .join("SKILL.md");

        if let Some(parent) = target_binary.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {} 失败: {e}", parent.display()))?;
        }
        copy_if_changed(&bundled_binary, &target_binary).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => format!(
                "打包资源缺少 blowup-mcp 二进制(预期 {})。请用 `just build-mcp && just build` 重新打包",
                bundled_binary.display()
            ),
            _ => format!("copy blowup-mcp 失败: {e}"),
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&target_binary, perms)
                .map_err(|e| format!("chmod 0755 失败: {e}"))?;
        }

        let home = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .ok_or_else(|| "no HOME env var".to_string())?;
        let skill_dir = home
            .join(".claude")
            .join("skills")
            .join("blowup-wiki-writer");
        std::fs::create_dir_all(&skill_dir)
            .map_err(|e| format!("mkdir {} 失败: {e}", skill_dir.display()))?;
        let skill_target = skill_dir.join("SKILL.md");
        std::fs::copy(&bundled_skill, &skill_target).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => format!(
                "打包资源缺少 SKILL.md(预期 {})",
                bundled_skill.display()
            ),
            _ => format!("copy SKILL.md 失败: {e}"),
        })?;

        // `--scope user` registers the MCP server in the user-global
        // Claude Code config, not the per-project "local" default —
        // the skill should be available in every project directory,
        // not just wherever the desktop app happened to be launched.
        let manual_command = format!(
            "claude mcp add --scope user blowup-skill {}",
            shell_escape(&target_binary_str)
        );

        let claude_added = std::process::Command::new("claude")
            .args([
                "mcp",
                "add",
                "--scope",
                "user",
                "blowup-skill",
                &target_binary_str,
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        Ok(InstallReport {
            binary_path: target_binary_str,
            skill_path: skill_target.to_string_lossy().into_owned(),
            claude_added,
            manual_command: if claude_added {
                None
            } else {
                Some(manual_command)
            },
        })
    })
    .await
    .map_err(|e| format!("install task panicked: {e}"))??;

    tracing::info!(
        binary = %report.binary_path,
        skill = %report.skill_path,
        claude_added = report.claude_added,
        "skill bridge installed"
    );

    Ok(report)
}

/// Copy `src` to `dst` unless the contents are already identical.
/// Uses a length pre-check so we only read the file bytes when they
/// actually need hashing — the common re-install case (same binary)
/// hashes, the upgrade case (different length) copies directly.
fn copy_if_changed(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    let src_meta = std::fs::metadata(src)?;
    let dst_meta = std::fs::metadata(dst).ok();
    let needs_copy = match dst_meta {
        None => true,
        Some(m) if m.len() != src_meta.len() => true,
        Some(_) => file_sha256(src)? != file_sha256(dst)?,
    };
    if needs_copy {
        std::fs::copy(src, dst)?;
    }
    Ok(())
}

fn file_sha256(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hasher.finalize().to_vec())
}
