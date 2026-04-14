//! Socket path resolution and connection helper.
//!
//! Production code uses `default_socket_path()`; tests and debugging
//! can override the path via the `BLOWUP_MCP_SOCKET_OVERRIDE`
//! environment variable. The bridge binary and the desktop app's
//! Tauri command MUST both use `resolve_socket_path()` so they agree
//! on the same location (with or without the override).

use std::path::PathBuf;

const ENV_OVERRIDE: &str = "BLOWUP_MCP_SOCKET_OVERRIDE";

/// Default socket path on this OS. macOS: app-data dir; Linux: runtime
/// dir (auto-cleaned by systemd-tmpfiles), with HOME/.local/share
/// fallback.
#[cfg(target_os = "macos")]
pub fn default_socket_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("blowup")
        .join("skill.sock")
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn default_socket_path() -> PathBuf {
    if let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(rt).join("blowup").join("skill.sock");
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("blowup")
        .join("skill.sock")
}

#[cfg(not(unix))]
pub fn default_socket_path() -> PathBuf {
    // Windows / other targets are not yet supported — see the plan
    // header. We still return *something* so callers compile, but
    // the bridge and the desktop will both refuse to use it.
    PathBuf::from("blowup-skill-unsupported")
}

/// Returns `BLOWUP_MCP_SOCKET_OVERRIDE` if set and non-empty,
/// otherwise `default_socket_path()`. **Always** call this — never
/// `default_socket_path()` directly — so tests can inject a tempdir.
pub fn resolve_socket_path() -> PathBuf {
    if let Ok(s) = std::env::var(ENV_OVERRIDE) {
        if !s.is_empty() {
            return PathBuf::from(s);
        }
    }
    default_socket_path()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn override_takes_precedence() {
        // SAFETY: the serial_test attribute makes this section single-
        // threaded so set_var/remove_var do not race with other tests.
        unsafe {
            std::env::set_var(ENV_OVERRIDE, "/tmp/custom.sock");
        }
        assert_eq!(resolve_socket_path(), PathBuf::from("/tmp/custom.sock"));
        unsafe {
            std::env::remove_var(ENV_OVERRIDE);
        }
    }

    #[test]
    #[serial]
    fn empty_override_falls_back_to_default() {
        unsafe {
            std::env::set_var(ENV_OVERRIDE, "");
        }
        let p = resolve_socket_path();
        assert_ne!(p, PathBuf::from(""));
        assert!(p.to_string_lossy().contains("skill.sock"));
        unsafe {
            std::env::remove_var(ENV_OVERRIDE);
        }
    }

    #[test]
    #[serial]
    fn unset_override_uses_default() {
        unsafe {
            std::env::remove_var(ENV_OVERRIDE);
        }
        let p = resolve_socket_path();
        assert!(p.to_string_lossy().contains("skill.sock"));
    }
}
