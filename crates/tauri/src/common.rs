//! Tauri-specific UI helpers. Platform-agnostic helpers (exec_command,
//! path utilities, file reading, director-name normalization) live in
//! `blowup_core::infra::common` and are re-exported here so existing
//! `crate::common::*` imports keep working.

pub use blowup_core::infra::common::*;

/// Generate a unique window label with a timestamp suffix.
pub fn unique_window_label(prefix: &str) -> String {
    format!(
        "{prefix}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    )
}

/// Open a new WebviewWindow. On Windows, runs on main thread to avoid WebView2 deadlock.
#[cfg(target_os = "windows")]
pub fn open_child_window(
    app: &tauri::AppHandle,
    label: &str,
    url: &str,
    title: &str,
    size: (f64, f64),
    min_size: Option<(f64, f64)>,
) -> std::result::Result<(), String> {
    let app2 = app.clone();
    let label = label.to_string();
    let url = url.to_string();
    let title = title.to_string();
    app.run_on_main_thread(move || {
        let mut builder =
            tauri::WebviewWindowBuilder::new(&app2, &label, tauri::WebviewUrl::App(url.into()))
                .title(&title)
                .inner_size(size.0, size.1);
        if let Some((w, h)) = min_size {
            builder = builder.min_inner_size(w, h);
        }
        if let Err(e) = builder.build() {
            tracing::error!(error = %e, label, "创建子窗口失败");
        }
    })
    .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn open_child_window(
    app: &tauri::AppHandle,
    label: &str,
    url: &str,
    title: &str,
    size: (f64, f64),
    min_size: Option<(f64, f64)>,
) -> std::result::Result<(), String> {
    let mut builder =
        tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App(url.into()))
            .title(title)
            .inner_size(size.0, size.1);
    if let Some((w, h)) = min_size {
        builder = builder.min_inner_size(w, h);
    }
    builder
        .build()
        .map_err(|e| format!("创建子窗口失败: {e}"))?;
    Ok(())
}
