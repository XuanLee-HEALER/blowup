use super::{
    PlayerState, TrackInfo, close_player, get_current_file_path, open_player, with_player,
};
use blowup_core::subtitle::parser::{SubtitleOverlayConfig, merge_to_ass, overlay_cache_key};
use tauri::Manager;

#[tauri::command]
pub fn cmd_open_player(app: tauri::AppHandle, file_path: String) -> Result<(), String> {
    tracing::info!(file_path, "cmd_open_player");
    open_player(&app, &file_path)
}

#[tauri::command]
pub fn cmd_close_player(app: tauri::AppHandle) -> Result<(), String> {
    tracing::info!("cmd_close_player");
    close_player(&app)
}

#[tauri::command]
pub fn cmd_player_play_pause() -> Result<(), String> {
    with_player(|p| {
        let pause_str = p.mpv.get_property_string("pause");
        let paused = pause_str.as_deref() == Some("yes");
        tracing::info!(pause_str = ?pause_str, paused, "cmd_player_play_pause: toggling");
        if paused {
            p.mpv.set_property_string("pause", "no")
        } else {
            p.mpv.set_property_string("pause", "yes")
        }
    })
}

#[tauri::command]
pub fn cmd_player_seek(position: f64) -> Result<(), String> {
    tracing::info!(position, "cmd_player_seek");
    with_player(|p| p.mpv.command(&["seek", &position.to_string(), "absolute"]))
}

#[tauri::command]
pub fn cmd_player_seek_relative(offset: f64) -> Result<(), String> {
    tracing::info!(offset, "cmd_player_seek_relative");
    with_player(|p| p.mpv.command(&["seek", &offset.to_string(), "relative"]))
}

#[tauri::command]
pub fn cmd_player_set_volume(volume: f64) -> Result<(), String> {
    tracing::info!(volume, "cmd_player_set_volume");
    with_player(|p| {
        p.mpv
            .set_property_double("volume", volume.clamp(0.0, 100.0))
    })
}

#[tauri::command]
pub fn cmd_player_get_state() -> Result<PlayerState, String> {
    let result = with_player(|p| Ok(p.get_state()));
    if let Ok(ref s) = result {
        tracing::debug!(
            playing = s.playing,
            paused = s.paused,
            pos = s.position,
            dur = s.duration,
            vol = s.volume,
            "cmd_player_get_state"
        );
    }
    result
}

#[tauri::command]
pub fn cmd_player_set_subtitle_track(track_id: i64) -> Result<(), String> {
    tracing::info!(track_id, "cmd_player_set_subtitle_track");
    with_player(|p| p.mpv.set_property_string("sid", &track_id.to_string()))
}

#[tauri::command]
pub fn cmd_player_set_audio_track(track_id: i64) -> Result<(), String> {
    tracing::info!(track_id, "cmd_player_set_audio_track");
    with_player(|p| p.mpv.set_property_string("aid", &track_id.to_string()))
}

#[tauri::command]
pub fn cmd_player_get_tracks() -> Result<Vec<TrackInfo>, String> {
    tracing::info!("cmd_player_get_tracks");
    with_player(|p| Ok(p.get_tracks()))
}

#[tauri::command]
pub fn cmd_player_toggle_fullscreen(app: tauri::AppHandle) -> Result<(), String> {
    tracing::info!("cmd_player_toggle_fullscreen");
    if let Some(window) = app.get_webview_window("player") {
        let is_fullscreen = window.is_fullscreen().map_err(|e| e.to_string())?;
        window
            .set_fullscreen(!is_fullscreen)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn cmd_player_get_current_file() -> Result<Option<String>, String> {
    Ok(get_current_file_path())
}

#[tauri::command]
pub fn cmd_player_sub_add(path: String) -> Result<(), String> {
    tracing::info!(path, "cmd_player_sub_add");
    with_player(|p| p.mpv.command(&["sub-add", &path]))
}

#[tauri::command]
pub fn cmd_player_load_overlay_subs(configs: Vec<SubtitleOverlayConfig>) -> Result<String, String> {
    if configs.is_empty() {
        with_player(|p| p.mpv.set_property_string("sid", "0"))?;
        return Ok(String::new());
    }

    // Expand ~ in paths
    let configs: Vec<SubtitleOverlayConfig> = configs
        .into_iter()
        .map(|mut c| {
            c.path = shellexpand::tilde(&c.path).into_owned();
            c
        })
        .collect();

    // Merge to ASS with per-source styles (even for single subtitle, for consistent styling)
    let film_dir = std::path::Path::new(&configs[0].path)
        .parent()
        .ok_or("invalid subtitle path")?;
    let cache_name = overlay_cache_key(&configs);
    let ass_path = film_dir.join(&cache_name);

    if !ass_path.exists() {
        let ass_content = merge_to_ass(&configs)?;
        std::fs::write(&ass_path, &ass_content).map_err(|e| format!("写入 ASS 文件失败: {}", e))?;
        tracing::info!(?ass_path, "generated overlay ASS");
    } else {
        tracing::info!(?ass_path, "using cached overlay ASS");
    }

    let ass_str = ass_path.to_string_lossy().to_string();

    with_player(|p| {
        p.mpv.set_property_string("sub-ass-override", "no")?;
        p.mpv.command(&["sub-add", &ass_str, "select"])?;
        Ok(())
    })?;

    Ok(ass_str)
}
