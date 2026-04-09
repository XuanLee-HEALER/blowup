use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session, SessionOptions,
    SessionPersistenceConfig,
};
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub type TorrentId = usize;
pub type TorrentHandle = Arc<ManagedTorrent>;

pub struct TorrentManager {
    session: Arc<Session>,
    semaphore: Arc<Semaphore>,
}

#[derive(Debug, Serialize)]
pub struct TorrentFileInfo {
    pub index: usize,
    pub name: String,
    pub size: u64,
}

impl TorrentManager {
    pub async fn new(
        library_root: PathBuf,
        max_concurrent: usize,
        enable_dht: bool,
        persist_session: bool,
        trackers: Vec<String>,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(&library_root).map_err(|e| e.to_string())?;

        let tracker_urls: HashSet<url::Url> =
            trackers.iter().filter_map(|t| t.parse().ok()).collect();

        let persistence = if persist_session {
            Some(SessionPersistenceConfig::Json { folder: None })
        } else {
            None
        };

        let opts = SessionOptions {
            disable_dht: !enable_dht,
            concurrent_init_limit: Some(max_concurrent),
            trackers: tracker_urls,
            fastresume: true,
            persistence,
            ..Default::default()
        };

        let session = Session::new_with_opts(library_root, opts)
            .await
            .map_err(|e| e.to_string())?;

        tracing::info!("torrent session initialized");

        Ok(Self {
            session,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        })
    }

    /// Start downloading a torrent. Returns (torrent_id, handle).
    ///
    /// `trackers` — optional extra tracker URLs to inject into this torrent
    /// (merged with session-level and torrent-embedded trackers by librqbit).
    pub async fn start_download(
        &self,
        target: &str,
        output_folder: PathBuf,
        only_files: Option<Vec<usize>>,
        trackers: Option<Vec<String>>,
    ) -> Result<(TorrentId, TorrentHandle), String> {
        let _permit = self.semaphore.acquire().await.map_err(|e| e.to_string())?;

        std::fs::create_dir_all(&output_folder).map_err(|e| e.to_string())?;

        let opts = AddTorrentOptions {
            output_folder: Some(output_folder.to_string_lossy().to_string()),
            only_files,
            overwrite: true,
            trackers,
            ..Default::default()
        };

        let response = self
            .session
            .add_torrent(AddTorrent::from_url(target), Some(opts))
            .await
            .map_err(|e| e.to_string())?;

        match response {
            AddTorrentResponse::Added(id, handle) => {
                tracing::info!(id, "torrent added");
                Ok((id, handle))
            }
            AddTorrentResponse::AlreadyManaged(id, handle) => {
                tracing::info!(id, "torrent already managed");
                Ok((id, handle))
            }
            AddTorrentResponse::ListOnly(_) => Err("unexpected list_only response".to_string()),
        }
    }

    /// Preview files in a torrent without downloading.
    pub async fn get_torrent_files(&self, target: &str) -> Result<Vec<TorrentFileInfo>, String> {
        let opts = AddTorrentOptions {
            list_only: true,
            ..Default::default()
        };

        let response = self
            .session
            .add_torrent(AddTorrent::from_url(target), Some(opts))
            .await
            .map_err(|e| e.to_string())?;

        match response {
            AddTorrentResponse::ListOnly(info) => {
                let iter = info.info.iter_file_details().map_err(|e| e.to_string())?;
                let files: Vec<TorrentFileInfo> = iter
                    .enumerate()
                    .map(|(i, f)| TorrentFileInfo {
                        index: i,
                        name: f.filename.to_string().unwrap_or_default(),
                        size: f.len,
                    })
                    .collect();
                Ok(files)
            }
            _ => Err("expected list_only response".to_string()),
        }
    }

    /// Pause a torrent.
    pub async fn pause(&self, id: TorrentId) -> Result<(), String> {
        let handle = self
            .get_handle(id)
            .ok_or_else(|| format!("torrent {id} not found"))?;
        self.session
            .pause(&handle)
            .await
            .map_err(|e| e.to_string())?;
        tracing::info!(id, "torrent paused");
        Ok(())
    }

    /// Resume a paused torrent.
    pub async fn unpause(&self, id: TorrentId) -> Result<(), String> {
        let handle = self
            .get_handle(id)
            .ok_or_else(|| format!("torrent {id} not found"))?;
        self.session
            .unpause(&handle)
            .await
            .map_err(|e| e.to_string())?;
        tracing::info!(id, "torrent resumed");
        Ok(())
    }

    /// Remove a torrent from session (keep files on disk).
    pub async fn remove(&self, id: TorrentId) -> Result<(), String> {
        self.session
            .delete(id.into(), false)
            .await
            .map_err(|e| e.to_string())?;
        tracing::info!(id, "torrent removed");
        Ok(())
    }

    /// Get handle for a torrent by ID.
    pub fn get_handle(&self, id: TorrentId) -> Option<TorrentHandle> {
        self.session.with_torrents(|iter| {
            for (tid, handle) in iter {
                if tid == id {
                    return Some(handle.clone());
                }
            }
            None
        })
    }

    /// Shutdown the session gracefully.
    pub fn shutdown(&self) {
        self.session.cancellation_token().cancel();
        tracing::info!("torrent session shutdown requested");
    }
}
