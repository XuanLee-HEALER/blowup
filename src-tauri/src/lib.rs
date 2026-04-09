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

            let mut cfg = config::load_config();
            config::resolve_tool_paths(&mut cfg);

            // Initialize library index
            let t0 = std::time::Instant::now();
            let root_dir = shellexpand::tilde(&cfg.library.root_dir).to_string();
            let library_root = std::path::PathBuf::from(&root_dir);
            std::fs::create_dir_all(&library_root).ok();
            let library_index = library_index::LibraryIndex::load(&library_root);
            handle.manage(library_index);
            tracing::info!(elapsed_ms = t0.elapsed().as_millis(), "library index loaded");

            // Allow asset protocol to serve files from the library directory
            if let Err(e) = app.asset_protocol_scope().allow_directory(&library_root, true) {
                tracing::warn!(error = %e, "failed to register library root in asset scope");
            }

            // Init tracker manager (loads trackers.json, migrates legacy format)
            let (tracker_mgr, trackers) = commands::tracker::TrackerManager::load();
            handle.manage(tracker_mgr);
            tracing::info!(count = trackers.len(), "tracker manager loaded");

            // Init DB (must complete before window opens — commands depend on pool)
            let t1 = std::time::Instant::now();
            tauri::async_runtime::block_on(async {
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
            });
            tracing::info!(elapsed_ms = t1.elapsed().as_millis(), "database initialized");

            // Mark stale 'downloading' records as 'paused' (crash recovery)
            tauri::async_runtime::block_on(async {
                let pool = handle.state::<sqlx::SqlitePool>();
                let res = sqlx::query(
                    "UPDATE downloads SET status='paused' WHERE status='downloading'",
                )
                .execute(pool.inner())
                .await;
                match res {
                    Ok(r) if r.rows_affected() > 0 => {
                        tracing::info!(count = r.rows_affected(), "paused stale downloads");
                    }
                    Err(e) => tracing::warn!(error = %e, "failed to pause stale downloads"),
                    _ => {}
                }
            });

            // Init torrent manager in background — don't block window creation
            let tm_handle = handle.clone();
            let t2 = std::time::Instant::now();
            tauri::async_runtime::spawn(async move {
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
                        tracing::info!(elapsed_ms = t2.elapsed().as_millis(), "torrent manager ready");
                        tm_handle.manage(tm);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, elapsed_ms = t2.elapsed().as_millis(), "failed to init torrent manager");
                    }
                }
            });

            // Background tracker auto-refresh (daily, with staleness check on startup)
            let tracker_state = handle.state::<commands::tracker::TrackerManager>().inner().clone();
            tauri::async_runtime::spawn(async move {
                if tracker_state.is_stale().await {
                    tracing::info!("tracker list stale, refreshing");
                    if let Err(e) = tracker_state.refresh_auto().await {
                        tracing::warn!(error = %e, "startup tracker refresh failed");
                    }
                }
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(24 * 3600)).await;
                    tracing::info!("periodic tracker refresh");
                    if let Err(e) = tracker_state.refresh_auto().await {
                        tracing::warn!(error = %e, "periodic tracker refresh failed");
                    }
                }
            });

            // Open devtools in debug builds
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").expect("main window");
                window.open_devtools();
            }

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
            commands::tmdb::enrich_index_entry,
            // Download
            commands::download::get_torrent_files,
            commands::download::start_download,
            commands::download::list_downloads,
            commands::download::pause_download,
            commands::download::resume_download,
            commands::download::delete_download,
            commands::download::redownload,
            commands::download::list_download_existing_files,
            commands::tracker::get_tracker_status,
            commands::tracker::refresh_trackers,
            commands::tracker::add_user_trackers,
            // Subtitle & media
            commands::subtitle::fetch_subtitle_cmd,
            commands::subtitle::align_subtitle_cmd,
            commands::subtitle::extract_subtitle_cmd,
            commands::subtitle::list_subtitle_streams_cmd,
            commands::subtitle::shift_subtitle_cmd,
            commands::media::probe_media,
            commands::media::probe_media_detail,
            commands::media::probe_and_cache,
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
            // Knowledge base — entries
            commands::library::entries::list_entries,
            commands::library::entries::get_entry,
            commands::library::entries::create_entry,
            commands::library::entries::update_entry_name,
            commands::library::entries::update_entry_wiki,
            commands::library::entries::delete_entry,
            commands::library::entries::add_entry_tag,
            commands::library::entries::remove_entry_tag,
            commands::library::entries::list_all_tags,
            commands::library::entries::add_relation,
            commands::library::entries::remove_relation,
            commands::library::entries::list_relation_types,
            // Knowledge base — graph
            commands::library::graph::get_graph_data,
            // Library items
            commands::library::items::add_library_item,
            commands::library::items::list_library_items,
            commands::library::items::get_library_item,
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
            commands::library::items::delete_library_resource,
            commands::library::items::refresh_index_entry,
            commands::library::items::delete_film_directory,
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
            player::commands::cmd_player_get_current_file,
            player::commands::cmd_player_sub_add,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|handle, event| {
            if let tauri::RunEvent::Exit = event {
                cache::flush_cache();
                if let Some(idx) = handle.try_state::<library_index::LibraryIndex>() {
                    idx.flush();
                }
                // Pause active downloads before shutting down torrent session
                if let Some(pool) = handle.try_state::<sqlx::SqlitePool>() {
                    tauri::async_runtime::block_on(async {
                        sqlx::query(
                            "UPDATE downloads SET status='paused' WHERE status='downloading'",
                        )
                        .execute(pool.inner())
                        .await
                        .ok();
                    });
                }
                if let Some(tm) = handle.try_state::<torrent::TorrentManager>() {
                    tm.shutdown();
                }
            }
        });
}
