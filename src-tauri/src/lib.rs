pub mod commands;
pub mod common;
pub mod config;
pub mod db;
pub mod error;
pub mod ffmpeg;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                let pool = db::init_db(&handle)
                    .await
                    .expect("Failed to initialize database");
                handle.manage(pool);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search::search_yify_cmd,
            commands::tmdb::search_movies,
            commands::tmdb::discover_movies,
            commands::tmdb::list_genres,
            commands::download::download_target,
            commands::tracker::update_trackers,
            commands::subtitle::fetch_subtitle_cmd,
            commands::subtitle::align_subtitle_cmd,
            commands::subtitle::extract_subtitle_cmd,
            commands::subtitle::list_subtitle_streams_cmd,
            commands::subtitle::shift_subtitle_cmd,
            commands::media::probe_media,
            commands::config::get_config,
            commands::config::set_config_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
