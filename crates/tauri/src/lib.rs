pub mod commands;
pub mod common;
pub mod player;
pub mod skill_bridge;

use blowup_core::AppContext;
use blowup_core::config;
use blowup_core::infra::events::EventBus;
use blowup_core::infra::{cache, db};
use blowup_core::library::index::LibraryIndex;
use blowup_core::tasks::TaskRegistry;
use blowup_core::torrent::manager::TorrentManager;
use blowup_core::torrent::tracker::TrackerManager;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::OnceCell;

/// Local bind address for the in-process HTTP server. Both the frontend
/// (via the Tauri IPC bridge) and LAN-side iOS/iPad clients reach the
/// same `blowup-core` through this router — see docs/REFACTOR.md
/// step 5. The port can later be surfaced as a user setting.
const EMBEDDED_SERVER_BIND: &str = "127.0.0.1:17690";

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("blowup_lib=debug"));
    fmt().with_env_filter(filter).init();
}

/// Resolve the embedded HTTP server's bearer token. The Tauri frontend
/// doesn't hit the HTTP server (it uses IPC), so this token is only
/// relevant to LAN/iOS clients — but it still has to exist because
/// every route is gated. Prefer `$BLOWUP_SERVER_TOKEN` when set;
/// otherwise generate a fresh random token and log it so the user
/// can copy it to their external client.
fn resolve_server_auth_token() -> String {
    match std::env::var("BLOWUP_SERVER_TOKEN") {
        Ok(t) if !t.is_empty() => {
            tracing::info!("embedded server: auth token loaded from BLOWUP_SERVER_TOKEN");
            t
        }
        _ => {
            let t = blowup_server::auth::generate_random_token();
            tracing::warn!(
                token = %t,
                "embedded server: BLOWUP_SERVER_TOKEN not set — generated ephemeral token for this session"
            );
            t
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_start = std::time::Instant::now();
    init_tracing();
    tracing::debug!(
        "[timing] tracing init done: {}ms",
        app_start.elapsed().as_millis()
    );
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            tracing::debug!(
                "[timing] setup start: {}ms",
                app_start.elapsed().as_millis()
            );
            let handle = app.handle().clone();
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("could not resolve app data dir");
            config::init_app_data_dir(data_dir);
            cache::init_cache();

            let cfg = config::load_config();
            tracing::debug!(
                "[timing] config loaded: {}ms",
                app_start.elapsed().as_millis()
            );

            // Resolve tool paths in background — which() is slow on Windows
            let mut cfg_bg = cfg.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if config::resolve_tool_paths(&mut cfg_bg) {
                    tracing::info!("tool paths resolved in background");
                }
            });

            // Initialize library index (sync file read, no write-back on load)
            let root_dir = shellexpand::tilde(&cfg.library.root_dir).to_string();
            let library_root = std::path::PathBuf::from(&root_dir);
            std::fs::create_dir_all(&library_root).ok();
            let library_index = Arc::new(LibraryIndex::load(&library_root));
            handle.manage(library_index.clone());
            tracing::debug!(
                "[timing] library index loaded: {}ms",
                app_start.elapsed().as_millis()
            );

            // Allow asset protocol to serve files from the library directory
            if let Err(e) = app
                .asset_protocol_scope()
                .allow_directory(&library_root, true)
            {
                tracing::warn!(error = %e, "failed to register library root in asset scope");
            }

            // Init tracker manager (loads trackers.json, migrates legacy format)
            let (tracker_mgr, trackers) = TrackerManager::load();
            let tracker_arc = Arc::new(tracker_mgr.clone());
            handle.manage(tracker_mgr);
            tracing::debug!(
                "[timing] tracker manager loaded: {}ms",
                app_start.elapsed().as_millis()
            );

            // Shared EventBus — Tauri wrappers publish domain events here and a
            // listener task re-emits them via app.emit for the frontend; the
            // embedded server's SSE endpoint subscribes to the same bus so LAN
            // clients see the exact same notifications.
            let events = EventBus::new();
            handle.manage(events.clone());

            // Long-running task registry (subtitle alignment etc.) — see
            // core::tasks. Tauri command wrappers + embedded server share
            // this single instance so state is consistent across clients.
            let tasks = TaskRegistry::new();
            handle.manage(tasks.clone());
            {
                let app_for_events = handle.clone();
                let mut rx = events.subscribe();
                tauri::async_runtime::spawn(async move {
                    // Loop forever: on Lagged we drop the skipped events and
                    // keep going; only Closed terminates the forwarder.
                    // Using `while let Ok(..)` here would exit on the first
                    // Lagged and silently freeze the frontend.
                    loop {
                        match rx.recv().await {
                            Ok(event) => {
                                if let Err(e) = app_for_events.emit(event.as_str(), ()) {
                                    tracing::warn!(error = %e, event = event.as_str(),
                                        "failed to forward event bus → app.emit");
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                tracing::warn!(
                                    skipped = n,
                                    "event bus → app.emit forwarder lagged"
                                );
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                tracing::info!("event bus closed — forwarder exiting");
                                break;
                            }
                        }
                    }
                });
            }

            // OnceCell that holds the async-initialized TorrentManager.
            // Both Tauri wrappers (via State<Arc<OnceCell<…>>>) and the
            // embedded server's AppContext read from it.
            let torrent_cell: Arc<OnceCell<TorrentManager>> = Arc::new(OnceCell::new());
            handle.manage(torrent_cell.clone());

            // Init DB + crash-recovery in a single block_on
            tracing::debug!(
                "[timing] db init start: {}ms",
                app_start.elapsed().as_millis()
            );
            let db_data_dir = app
                .path()
                .app_data_dir()
                .expect("could not resolve app data dir for db");
            let pool = tauri::async_runtime::block_on(async {
                match db::init_db(&db_data_dir).await {
                    Ok(pool) => {
                        // Mark stale 'downloading' records as 'paused' (crash recovery)
                        let res = sqlx::query(
                            "UPDATE downloads SET status='paused' WHERE status='downloading'",
                        )
                        .execute(&pool)
                        .await;
                        if let Ok(r) = res
                            && r.rows_affected() > 0
                        {
                            tracing::info!(count = r.rows_affected(), "paused stale downloads");
                        }
                        pool
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
            handle.manage(pool.clone());

            // Resolve the shared bearer token once, up-front. The Tauri
            // frontend doesn't call the HTTP server (Tauri IPC is used
            // instead), so this is only relevant to LAN/iOS clients —
            // but we need a value now so the AppContext is complete.
            let auth_token = Arc::new(resolve_server_auth_token());

            // Build the canonical AppContext once. Both the embedded
            // axum server and any future in-process caller read the
            // same struct, so there's exactly one place to extend when
            // a new shared resource shows up.
            let ctx = Arc::new(AppContext::new(
                pool,
                library_index.clone(),
                tracker_arc.clone(),
                torrent_cell.clone(),
                events.clone(),
                tasks.clone(),
                auth_token,
            ));
            handle.manage(ctx.clone());
            handle.manage(crate::skill_bridge::state::SkillBridgeState::new());

            tracing::debug!(
                "[timing] setup complete: {}ms",
                app_start.elapsed().as_millis()
            );

            // NOTE: no `window.open_devtools()` here. The main window
            // starts hidden (`visible:false` in tauri.conf.json) and
            // only becomes visible via `close_splashscreen`. Opening
            // devtools on a hidden WKWebView races with the later
            // `show()` call and crashes the app on macOS. Open
            // devtools manually with the shortcut (⌘⌥I) once the
            // main window is up.

            // Init torrent manager in background — don't block window.
            // On success, also start the embedded HTTP server for LAN clients.
            let tm_handle = handle.clone();
            let ctx_for_bg = ctx.clone();
            tauri::async_runtime::spawn(async move {
                let t = std::time::Instant::now();
                match TorrentManager::new(
                    library_root,
                    cfg.download.max_concurrent,
                    cfg.download.enable_dht,
                    cfg.download.persist_session,
                    trackers,
                )
                .await
                {
                    Ok(tm) => {
                        tracing::info!(
                            elapsed_ms = t.elapsed().as_millis(),
                            "torrent manager ready"
                        );
                        let _ = ctx_for_bg.torrent.set(tm.clone());
                        tm_handle.manage(tm);
                    }
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            elapsed_ms = t.elapsed().as_millis(),
                            "failed to init torrent manager"
                        );
                    }
                }

                // Start the embedded blowup-server axum router bound to
                // 127.0.0.1. It shares the AppContext instance with
                // the Tauri wrappers — no duplicate wiring.
                tracing::info!(
                    bind = EMBEDDED_SERVER_BIND,
                    "embedded blowup-server listening"
                );
                if let Err(e) =
                    blowup_server::serve(EMBEDDED_SERVER_BIND, (*ctx_for_bg).clone()).await
                {
                    tracing::warn!(
                        error = %e,
                        bind = EMBEDDED_SERVER_BIND,
                        "embedded server exited"
                    );
                }
            });

            // Background tracker auto-refresh (daily, with staleness check on startup)
            let tracker_state = handle.state::<TrackerManager>().inner().clone();
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

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Search & discovery
            commands::search::search_movie_cmd,
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
            // Audio
            commands::audio::list_audio_streams_cmd,
            commands::audio::extract_audio_cmd,
            commands::audio::get_audio_peaks,
            commands::audio::open_waveform_window,
            // Subtitle & media
            commands::subtitle::fetch_subtitle_cmd,
            commands::subtitle::align_subtitle_cmd,
            commands::subtitle::extract_subtitle_cmd,
            commands::subtitle::list_subtitle_streams_cmd,
            commands::subtitle::shift_subtitle_cmd,
            commands::subtitle::align_to_audio_cmd,
            commands::subtitle::parse_subtitle_cmd,
            commands::subtitle::open_subtitle_viewer,
            commands::subtitle::search_subtitles_cmd,
            commands::subtitle::download_subtitle_cmd,
            // Tasks
            commands::tasks::list_tasks,
            commands::tasks::dismiss_task,
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
            commands::library::items::save_subtitle_configs,
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
            player::commands::cmd_player_load_overlay_subs,
            // Skill bridge
            commands::skill::skill_bridge_status,
            commands::skill::skill_bridge_start,
            commands::skill::skill_bridge_stop,
            commands::skill::skill_bridge_get_install_snippets,
            commands::skill::skill_bridge_install_to_claude_code,
            // Splash
            commands::splash::close_splashscreen,
        ])
        .on_window_event(|window, event| {
            // Only act on the main window — closing the player popout
            // or a waveform/subtitle viewer should NOT tear down the
            // skill bridge. Tauri assigns the label "main" to the
            // first unlabeled window in tauri.conf.json, which matches
            // the existing get_webview_window("main") calls in setup.
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { .. } = event
                && let Some(state) =
                    window.try_state::<crate::skill_bridge::state::SkillBridgeState>()
            {
                state.shutdown_blocking();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|handle, event| {
            if let tauri::RunEvent::Exit = event {
                cache::flush_cache();
                if let Some(idx) = handle.try_state::<Arc<LibraryIndex>>() {
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
                if let Some(tm) = handle.try_state::<TorrentManager>() {
                    tm.shutdown();
                }
            }
        });
}
