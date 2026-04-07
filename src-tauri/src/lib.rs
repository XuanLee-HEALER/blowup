pub mod cache;
pub mod commands;
pub mod common;
pub mod config;
pub mod db;
pub mod error;
pub mod ffmpeg;
pub mod library_index;
pub mod player;
pub mod torrent;

use tauri::Manager;

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("blowup_lib=debug"));
    fmt().with_env_filter(filter).init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("could not resolve app data dir");
            config::init_app_data_dir(data_dir);
            cache::init_cache();

            let cfg = config::load_config();

            // Initialize library index
            let root_dir = shellexpand::tilde(&cfg.library.root_dir).to_string();
            let library_root = std::path::PathBuf::from(&root_dir);
            std::fs::create_dir_all(&library_root).ok();
            let library_index = library_index::LibraryIndex::load(&library_root);
            handle.manage(library_index);

            // Initialize torrent manager + DB
            let trackers = commands::tracker::load_trackers();
            tauri::async_runtime::block_on(async move {
                // Init DB
                match db::init_db(&handle).await {
                    Ok(pool) => {
                        handle.manage(pool);
                    }
                    Err(msg) => {
                        use tauri_plugin_dialog::DialogExt;
                        handle
                            .dialog()
                            .message(msg)
                            .title("blowup 启动失败")
                            .blocking_show();
                        std::process::exit(1);
                    }
                }

                // Init torrent manager
                match torrent::TorrentManager::new(
                    library_root,
                    cfg.download.max_concurrent,
                    cfg.download.enable_dht,
                    cfg.download.persist_session,
                    trackers,
                )
                .await
                {
                    Ok(tm) => {
                        handle.manage(tm);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to init torrent manager");
                        // Non-fatal: download won't work but app still starts
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Search & discovery
            commands::search::search_yify_cmd,
            commands::tmdb::search_movies,
            commands::tmdb::discover_movies,
            commands::tmdb::list_genres,
            commands::tmdb::get_tmdb_movie_credits,
            commands::tmdb::enrich_movie_credits,
            // Download
            commands::download::get_torrent_files,
            commands::download::start_download,
            commands::download::list_downloads,
            commands::download::pause_download,
            commands::download::resume_download,
            commands::download::delete_download,
            commands::download::redownload,
            commands::tracker::update_trackers,
            // Subtitle & media
            commands::subtitle::fetch_subtitle_cmd,
            commands::subtitle::align_subtitle_cmd,
            commands::subtitle::extract_subtitle_cmd,
            commands::subtitle::list_subtitle_streams_cmd,
            commands::subtitle::shift_subtitle_cmd,
            commands::media::probe_media,
            commands::media::probe_media_detail,
            commands::media::open_in_player,
            // Config
            commands::config::get_config,
            commands::config::save_config_cmd,
            commands::config::get_cache_path,
            commands::export::export_knowledge_base,
            commands::export::import_knowledge_base,
            commands::export::export_config,
            commands::export::import_config,
            commands::export::export_knowledge_base_s3,
            commands::export::import_knowledge_base_s3,
            commands::export::export_config_s3,
            commands::export::import_config_s3,
            commands::export::test_s3_connection,
            // Knowledge base — people
            commands::library::people::list_people,
            commands::library::people::get_person,
            commands::library::people::create_person,
            commands::library::people::update_person_wiki,
            commands::library::people::delete_person,
            commands::library::people::add_person_relation,
            commands::library::people::remove_person_relation,
            // Knowledge base — films
            commands::library::films::list_films,
            commands::library::films::get_film,
            commands::library::films::add_film_from_tmdb,
            commands::library::films::update_film_wiki,
            commands::library::films::delete_film,
            commands::library::films::list_films_filtered,
            // Knowledge base — genres
            commands::library::genres::list_genres_tree,
            commands::library::genres::get_genre,
            commands::library::genres::create_genre,
            commands::library::genres::update_genre_wiki,
            commands::library::genres::delete_genre,
            commands::library::genres::link_film_genre,
            commands::library::genres::unlink_film_genre,
            commands::library::genres::link_person_genre,
            commands::library::genres::unlink_person_genre,
            // Knowledge base — reviews
            commands::library::reviews::add_review,
            commands::library::reviews::update_review,
            commands::library::reviews::delete_review,
            // Knowledge base — graph
            commands::library::graph::get_graph_data,
            // Library items
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
            // Library index
            commands::library::items::list_index_entries,
            commands::library::items::list_index_by_director,
            commands::library::items::search_index,
            commands::library::items::rebuild_index,
            // Player
            player::commands::cmd_open_player,
            player::commands::cmd_close_player,
            player::commands::cmd_player_play_pause,
            player::commands::cmd_player_seek,
            player::commands::cmd_player_seek_relative,
            player::commands::cmd_player_set_volume,
            player::commands::cmd_player_get_state,
            player::commands::cmd_player_set_subtitle_track,
            player::commands::cmd_player_set_audio_track,
            player::commands::cmd_player_get_tracks,
            player::commands::cmd_player_toggle_fullscreen,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|handle, event| {
            if let tauri::RunEvent::Exit = event {
                cache::flush_cache();
                if let Some(idx) = handle.try_state::<library_index::LibraryIndex>() {
                    idx.flush();
                }
                if let Some(tm) = handle.try_state::<torrent::TorrentManager>() {
                    tm.shutdown();
                }
            }
        });
}
