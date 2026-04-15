use tauri::Manager;

/// Called by the splash window once its animation has played out.
/// Closes the splash overlay and then reveals the main window. The
/// main window boots with `visible:false` in tauri.conf.json so the
/// user only ever sees splash → fade → main, never both at once.
#[tauri::command]
pub async fn close_splashscreen(app: tauri::AppHandle) {
    if let Some(splash) = app.get_webview_window("splashscreen") {
        let _ = splash.close();
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
}
