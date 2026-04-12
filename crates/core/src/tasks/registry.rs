use crate::tasks::model::{TaskKind, TaskRecord, TaskStatus};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Thread-safe in-memory store of task records. Cheap to clone (one
/// `Arc` bump) so it can be passed into `tokio::spawn` or handed to
/// axum state alongside Tauri state.
#[derive(Clone, Default)]
pub struct TaskRegistry {
    inner: Arc<RwLock<HashMap<String, TaskRecord>>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Try to start a new task. Fails if a task with the same id is
    /// already running — we refuse to kick off a concurrent
    /// alignment of the same SRT, which would race on the same
    /// output file.
    pub async fn start(&self, kind: TaskKind) -> Result<TaskRecord, String> {
        let id = kind.id().to_string();
        let mut guard = self.inner.write().await;
        if let Some(existing) = guard.get(&id)
            && !existing.status.is_terminal()
        {
            return Err(format!("task already running: {id}"));
        }
        let now = Utc::now();
        let record = TaskRecord {
            id: id.clone(),
            kind,
            status: TaskStatus::Running,
            started_at: now,
            updated_at: now,
        };
        guard.insert(id, record.clone());
        Ok(record)
    }

    pub async fn complete(&self, id: &str, summary: String, output_path: Option<String>) {
        if let Some(rec) = self.inner.write().await.get_mut(id) {
            rec.status = TaskStatus::Completed {
                summary,
                output_path,
            };
            rec.updated_at = Utc::now();
        }
    }

    pub async fn fail(&self, id: &str, error: String) {
        if let Some(rec) = self.inner.write().await.get_mut(id) {
            rec.status = TaskStatus::Failed { error };
            rec.updated_at = Utc::now();
        }
    }

    /// Explicit "user clicked dismiss" — remove the entry even if
    /// it's still running (the background tokio task keeps going
    /// and its update will land into an empty slot, which is fine).
    pub async fn dismiss(&self, id: &str) -> bool {
        self.inner.write().await.remove(id).is_some()
    }

    /// Snapshot of all current records, newest first by updated_at.
    pub async fn list(&self) -> Vec<TaskRecord> {
        let guard = self.inner.read().await;
        let mut records: Vec<TaskRecord> = guard.values().cloned().collect();
        records.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        records
    }

    pub async fn get(&self, id: &str) -> Option<TaskRecord> {
        self.inner.read().await.get(id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_kind(srt: &str) -> TaskKind {
        TaskKind::SubtitleAlignToAudio {
            srt_path: srt.to_string(),
            audio_path: "/tmp/audio.m4a".to_string(),
        }
    }

    #[tokio::test]
    async fn start_and_list() {
        let reg = TaskRegistry::new();
        reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        reg.start(sample_kind("/tmp/b.srt")).await.unwrap();
        let records = reg.list().await;
        assert_eq!(records.len(), 2);
        assert!(matches!(records[0].status, TaskStatus::Running));
    }

    #[tokio::test]
    async fn duplicate_start_rejected_while_running() {
        let reg = TaskRegistry::new();
        reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        let err = reg.start(sample_kind("/tmp/a.srt")).await.unwrap_err();
        assert!(err.contains("already running"));
    }

    #[tokio::test]
    async fn duplicate_start_allowed_after_terminal() {
        let reg = TaskRegistry::new();
        reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        reg.complete("/tmp/a.srt", "done".into(), None).await;
        // Completed tasks don't block a restart
        reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        let rec = reg.get("/tmp/a.srt").await.unwrap();
        assert!(matches!(rec.status, TaskStatus::Running));
    }

    #[tokio::test]
    async fn complete_updates_status() {
        let reg = TaskRegistry::new();
        reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        reg.complete(
            "/tmp/a.srt",
            "aligned 42 cues".into(),
            Some("/tmp/a.aligned.srt".into()),
        )
        .await;
        let rec = reg.get("/tmp/a.srt").await.unwrap();
        match rec.status {
            TaskStatus::Completed {
                summary,
                output_path,
            } => {
                assert_eq!(summary, "aligned 42 cues");
                assert_eq!(output_path.as_deref(), Some("/tmp/a.aligned.srt"));
            }
            _ => panic!("expected Completed"),
        }
    }

    #[tokio::test]
    async fn fail_updates_status() {
        let reg = TaskRegistry::new();
        reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        reg.fail("/tmp/a.srt", "ffmpeg exploded".into()).await;
        let rec = reg.get("/tmp/a.srt").await.unwrap();
        assert!(matches!(rec.status, TaskStatus::Failed { .. }));
    }

    #[tokio::test]
    async fn dismiss_removes_entry() {
        let reg = TaskRegistry::new();
        reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        assert!(reg.dismiss("/tmp/a.srt").await);
        assert!(reg.get("/tmp/a.srt").await.is_none());
        // Dismissing a nonexistent id returns false
        assert!(!reg.dismiss("/tmp/nope.srt").await);
    }
}
