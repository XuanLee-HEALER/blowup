//! BitTorrent tracker list management.
//!
//! - Fetches the ngosang/trackerslist aggregate list daily
//! - Verifies reachability via UDP connect or HTTP HEAD
//! - Persists the result in `{app_data_dir}/trackers.json`
//! - Exposes a "hot list" used by `TorrentManager::new` as default trackers

use crate::config::app_data_dir;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

const TRACKER_URL: &str =
    "https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_all.txt";

/// BT UDP tracker connect handshake magic number.
const BT_UDP_MAGIC: u64 = 0x0417_2710_1980;
const BT_UDP_ACTION_CONNECT: u32 = 0;

const VERIFY_CONCURRENCY: usize = 20;
const VERIFY_TIMEOUT_SECS: u64 = 3;
const STALE_HOURS: i64 = 24;

// ── Persistence ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct TrackerStore {
    #[serde(default)]
    last_updated: Option<String>,
    #[serde(default)]
    auto: Vec<String>,
    #[serde(default)]
    user: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackerStatus {
    pub auto_count: usize,
    pub user_count: usize,
    pub total_count: usize,
    pub last_updated: Option<String>,
}

fn trackers_json_path() -> PathBuf {
    app_data_dir().join("trackers.json")
}

/// Legacy plain-text tracker file (pre-v2 format).
fn legacy_tracker_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("blowup")
        .join("trackers.txt")
}

fn persist_store(store: &TrackerStore) -> Result<(), String> {
    let path = trackers_json_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

fn rebuild_hot_list(store: &TrackerStore) -> Vec<String> {
    let mut set = HashSet::new();
    let mut list = Vec::new();
    for t in store.auto.iter().chain(store.user.iter()) {
        if set.insert(t.clone()) {
            list.push(t.clone());
        }
    }
    list
}

// ── TrackerManager ───────────────────────────────────────────────

struct TrackerManagerInner {
    hot_list: RwLock<Vec<String>>,
    store: RwLock<TrackerStore>,
}

#[derive(Clone)]
pub struct TrackerManager {
    inner: Arc<TrackerManagerInner>,
}

impl TrackerManager {
    /// Load tracker store from disk.  Returns (manager, initial hot list).
    ///
    /// If `trackers.json` does not exist but the legacy `trackers.txt` does,
    /// migrate its contents into `auto` (with `last_updated = None` so a
    /// fresh pull + verify happens on first startup).
    pub fn load() -> (Self, Vec<String>) {
        let path = trackers_json_path();
        let store = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str::<TrackerStore>(&s).ok())
                .unwrap_or_default()
        } else {
            let legacy = legacy_tracker_path();
            if legacy.exists() {
                let auto: Vec<String> = std::fs::read_to_string(&legacy)
                    .unwrap_or_default()
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(String::from)
                    .collect();
                let migrated = TrackerStore {
                    last_updated: None,
                    auto,
                    user: Vec::new(),
                };
                persist_store(&migrated).ok();
                tracing::info!("migrated legacy trackers.txt → trackers.json");
                migrated
            } else {
                TrackerStore::default()
            }
        };

        let hot = rebuild_hot_list(&store);
        let initial = hot.clone();
        let mgr = Self {
            inner: Arc::new(TrackerManagerInner {
                hot_list: RwLock::new(hot),
                store: RwLock::new(store),
            }),
        };
        (mgr, initial)
    }

    /// Get the current hot tracker list (auto ∪ user, deduplicated).
    pub async fn hot_trackers(&self) -> Vec<String> {
        self.inner.hot_list.read().await.clone()
    }

    /// Check whether the auto list is stale (never updated or older than 24h).
    pub async fn is_stale(&self) -> bool {
        let store = self.inner.store.read().await;
        match &store.last_updated {
            None => true,
            Some(ts) => match chrono::DateTime::parse_from_rfc3339(ts) {
                Ok(dt) => Utc::now().signed_duration_since(dt).num_hours() >= STALE_HOURS,
                Err(_) => true,
            },
        }
    }

    /// Fetch remote tracker list, verify reachability, update store.
    pub async fn refresh_auto(&self) -> Result<TrackerStatus, String> {
        tracing::info!("tracker refresh: fetching remote list");

        let text = reqwest::get(TRACKER_URL)
            .await
            .map_err(|e| format!("拉取 tracker 列表失败: {e}"))?
            .text()
            .await
            .map_err(|e| format!("读取响应失败: {e}"))?;

        let candidates: Vec<String> = text
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        tracing::info!(
            count = candidates.len(),
            "tracker refresh: verifying reachability"
        );
        let verified = verify_trackers(candidates).await;
        tracing::info!(count = verified.len(), "tracker refresh: verified");

        let mut store = self.inner.store.write().await;
        store.auto = verified;
        store.last_updated = Some(Utc::now().to_rfc3339());
        persist_store(&store)?;

        let hot = rebuild_hot_list(&store);
        let status = TrackerStatus {
            auto_count: store.auto.len(),
            user_count: store.user.len(),
            total_count: hot.len(),
            last_updated: store.last_updated.clone(),
        };
        drop(store);

        *self.inner.hot_list.write().await = hot;
        Ok(status)
    }

    /// Add user-supplied tracker URLs (newline-separated, append-only).
    /// Validates URL format but does NOT verify reachability.
    pub async fn add_user_trackers(&self, raw: String) -> Result<TrackerStatus, String> {
        let mut new_urls = Vec::new();
        let mut errors = Vec::new();

        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match url::Url::parse(trimmed) {
                Ok(u) => {
                    let scheme = u.scheme();
                    if scheme == "udp" || scheme == "http" || scheme == "https" {
                        new_urls.push(trimmed.to_string());
                    } else {
                        errors.push(format!("不支持的协议「{scheme}」: {trimmed}"));
                    }
                }
                Err(e) => {
                    errors.push(format!("无效的 tracker 地址「{trimmed}」: {e}"));
                }
            }
        }

        if !errors.is_empty() {
            return Err(errors.join("\n"));
        }
        if new_urls.is_empty() {
            return Err("没有有效的 tracker 地址".to_string());
        }

        let mut store = self.inner.store.write().await;
        let existing: HashSet<String> = store.user.iter().cloned().collect();
        for u in new_urls {
            if !existing.contains(&u) {
                store.user.push(u);
            }
        }
        persist_store(&store)?;

        let hot = rebuild_hot_list(&store);
        let status = TrackerStatus {
            auto_count: store.auto.len(),
            user_count: store.user.len(),
            total_count: hot.len(),
            last_updated: store.last_updated.clone(),
        };
        drop(store);

        *self.inner.hot_list.write().await = hot;
        Ok(status)
    }

    /// Return current tracker counts + last update time.
    pub async fn get_status(&self) -> TrackerStatus {
        let store = self.inner.store.read().await;
        let hot = self.inner.hot_list.read().await;
        TrackerStatus {
            auto_count: store.auto.len(),
            user_count: store.user.len(),
            total_count: hot.len(),
            last_updated: store.last_updated.clone(),
        }
    }
}

// ── Reachability verification ────────────────────────────────────

async fn verify_trackers(candidates: Vec<String>) -> Vec<String> {
    let semaphore = Arc::new(tokio::sync::Semaphore::new(VERIFY_CONCURRENCY));
    let mut handles = Vec::with_capacity(candidates.len());

    for tracker_url in candidates {
        let sem = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.ok()?;
            let reachable = verify_one(&tracker_url).await;
            if reachable { Some(tracker_url) } else { None }
        }));
    }

    let mut verified = Vec::new();
    for h in handles {
        if let Ok(Some(url)) = h.await {
            verified.push(url);
        }
    }
    verified
}

async fn verify_one(tracker_url: &str) -> bool {
    let timeout = std::time::Duration::from_secs(VERIFY_TIMEOUT_SECS);

    let Ok(parsed) = url::Url::parse(tracker_url) else {
        return false;
    };

    match parsed.scheme() {
        "udp" => verify_udp(&parsed, timeout).await,
        "http" | "https" => verify_http(tracker_url, timeout).await,
        _ => false,
    }
}

async fn verify_udp(parsed: &url::Url, timeout: std::time::Duration) -> bool {
    let host = match parsed.host_str() {
        Some(h) => h,
        None => return false,
    };
    let port = parsed.port().unwrap_or(80);
    let addr_str = format!("{host}:{port}");

    let addr = match tokio::task::spawn_blocking(move || addr_str.to_socket_addrs())
        .await
        .ok()
        .and_then(|r| r.ok())
        .and_then(|mut addrs| addrs.next())
    {
        Some(a) => a,
        None => return false,
    };

    let Ok(sock) = tokio::net::UdpSocket::bind("0.0.0.0:0").await else {
        return false;
    };

    // BT UDP connect packet: 8 bytes magic + 4 bytes action + 4 bytes transaction_id
    let transaction_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(42);
    let mut buf = [0u8; 16];
    buf[0..8].copy_from_slice(&BT_UDP_MAGIC.to_be_bytes());
    buf[8..12].copy_from_slice(&BT_UDP_ACTION_CONNECT.to_be_bytes());
    buf[12..16].copy_from_slice(&transaction_id.to_be_bytes());

    if sock.send_to(&buf, addr).await.is_err() {
        return false;
    }

    let mut resp = [0u8; 16];
    tokio::time::timeout(timeout, sock.recv_from(&mut resp))
        .await
        .is_ok()
}

async fn verify_http(tracker_url: &str, timeout: std::time::Duration) -> bool {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .unwrap_or_default();

    client.head(tracker_url).send().await.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuild_hot_list_deduplicates() {
        let store = TrackerStore {
            last_updated: None,
            auto: vec!["udp://a:1337".into(), "udp://b:1337".into()],
            user: vec!["udp://b:1337".into(), "udp://c:6969".into()],
        };
        let hot = rebuild_hot_list(&store);
        assert_eq!(hot.len(), 3);
        assert!(hot.contains(&"udp://a:1337".to_string()));
        assert!(hot.contains(&"udp://b:1337".to_string()));
        assert!(hot.contains(&"udp://c:6969".to_string()));
    }

    #[test]
    fn rebuild_hot_list_empty() {
        let store = TrackerStore::default();
        let hot = rebuild_hot_list(&store);
        assert!(hot.is_empty());
    }

    #[test]
    fn store_serialization_roundtrip() {
        let store = TrackerStore {
            last_updated: Some("2026-04-09T12:00:00+00:00".to_string()),
            auto: vec!["udp://tracker1:1337".into()],
            user: vec!["https://tracker2/announce".into()],
        };
        let json = serde_json::to_string(&store).unwrap();
        let parsed: TrackerStore = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.auto.len(), 1);
        assert_eq!(parsed.user.len(), 1);
        assert_eq!(parsed.last_updated, store.last_updated);
    }

    #[tokio::test]
    async fn add_user_trackers_validates_scheme() {
        let (mgr, _) = TrackerManager::load();
        let result = mgr
            .add_user_trackers("ftp://bad-scheme.com:1234".into())
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("不支持的协议"));
    }

    #[tokio::test]
    async fn add_user_trackers_validates_url() {
        let (mgr, _) = TrackerManager::load();
        let result = mgr.add_user_trackers("not a url at all".into()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("无效的 tracker 地址"));
    }

    #[tokio::test]
    async fn add_user_trackers_rejects_empty() {
        let (mgr, _) = TrackerManager::load();
        let result = mgr.add_user_trackers("  \n  \n  ".into()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("没有有效的"));
    }
}
