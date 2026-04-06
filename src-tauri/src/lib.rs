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
            // M1 commands
            commands::search::search_yify_cmd,
            commands::tmdb::search_movies,
            commands::tmdb::discover_movies,
            commands::tmdb::list_genres,
            commands::tmdb::get_tmdb_movie_credits,
            commands::download::download_target,
            commands::download::start_download,
            commands::download::list_downloads,
            commands::download::cancel_download,
            commands::download::delete_download_record,
            commands::tracker::update_trackers,
            commands::subtitle::fetch_subtitle_cmd,
            commands::subtitle::align_subtitle_cmd,
            commands::subtitle::extract_subtitle_cmd,
            commands::subtitle::list_subtitle_streams_cmd,
            commands::subtitle::shift_subtitle_cmd,
            commands::media::probe_media,
            commands::config::get_config,
            commands::config::set_config_key,
            commands::config::set_music_playlist,
            // M2 library — people
            commands::library::people::list_people,
            commands::library::people::get_person,
            commands::library::people::create_person,
            commands::library::people::update_person_wiki,
            commands::library::people::delete_person,
            commands::library::people::add_person_relation,
            commands::library::people::remove_person_relation,
            // M2 library — films
            commands::library::films::list_films,
            commands::library::films::get_film,
            commands::library::films::add_film_from_tmdb,
            commands::library::films::update_film_wiki,
            commands::library::films::delete_film,
            // M2 library — genres
            commands::library::genres::list_genres_tree,
            commands::library::genres::get_genre,
            commands::library::genres::create_genre,
            commands::library::genres::update_genre_wiki,
            commands::library::genres::delete_genre,
            commands::library::genres::link_film_genre,
            commands::library::genres::unlink_film_genre,
            commands::library::genres::link_person_genre,
            commands::library::genres::unlink_person_genre,
            // M2 library — reviews
            commands::library::reviews::add_review,
            commands::library::reviews::update_review,
            commands::library::reviews::delete_review,
            // M2 library — graph
            commands::library::graph::get_graph_data,
            // M3 library — items
            commands::library::items::add_library_item,
            commands::library::items::list_library_items,
            commands::library::items::get_library_item,
            commands::library::items::link_item_to_film,
            commands::library::items::unlink_item_from_film,
            commands::library::items::remove_library_item,
            commands::library::items::scan_library_directory,
            commands::library::items::add_library_asset,
            commands::library::items::remove_library_asset,
            commands::library::items::get_library_stats,
            // M3 library — films filter
            commands::library::films::list_films_filtered,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
