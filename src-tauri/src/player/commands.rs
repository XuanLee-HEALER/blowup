use super::{PlayerState, TrackInfo, close_player, open_player, with_player};
use tauri::Manager;

#[tauri::command]
pub fn cmd_open_player(app: tauri::AppHandle, file_path: String) -> Result<(), String> {
    open_player(&app, &file_path)
}

#[tauri::command]
pub fn cmd_close_player(app: tauri::AppHandle) -> Result<(), String> {
    close_player(&app)
}

#[tauri::command]
pub fn cmd_player_play_pause() -> Result<(), String> {
    with_player(|p| {
        let paused = p.mpv.get_property_double("pause").unwrap_or(0.0);
        if paused != 0.0 {
            p.mpv.set_property_string("pause", "no")
        } else {
            p.mpv.set_property_string("pause", "yes")
        }
    })
}

#[tauri::command]
pub fn cmd_player_seek(position: f64) -> Result<(), String> {
    with_player(|p| p.mpv.command(&["seek", &position.to_string(), "absolute"]))
}

#[tauri::command]
pub fn cmd_player_seek_relative(offset: f64) -> Result<(), String> {
    with_player(|p| p.mpv.command(&["seek", &offset.to_string(), "relative"]))
}

#[tauri::command]
pub fn cmd_player_set_volume(volume: f64) -> Result<(), String> {
    with_player(|p| {
        p.mpv
            .set_property_double("volume", volume.clamp(0.0, 100.0))
    })
}

#[tauri::command]
pub fn cmd_player_get_state() -> Result<PlayerState, String> {
    with_player(|p| Ok(p.get_state()))
}

#[tauri::command]
pub fn cmd_player_set_subtitle_track(track_id: i64) -> Result<(), String> {
    with_player(|p| p.mpv.set_property_string("sid", &track_id.to_string()))
}

#[tauri::command]
pub fn cmd_player_set_audio_track(track_id: i64) -> Result<(), String> {
    with_player(|p| p.mpv.set_property_string("aid", &track_id.to_string()))
}

#[tauri::command]
pub fn cmd_player_get_tracks() -> Result<Vec<TrackInfo>, String> {
    with_player(|p| Ok(p.get_tracks()))
}

#[tauri::command]
pub fn cmd_player_toggle_fullscreen(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("player") {
        let is_fullscreen = window.is_fullscreen().map_err(|e| e.to_string())?;
        window
            .set_fullscreen(!is_fullscreen)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
