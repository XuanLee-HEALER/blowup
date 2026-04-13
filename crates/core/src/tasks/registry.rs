use crate::tasks::model::{TaskKind, TaskRecord, TaskStatus};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;

/// Thread-safe in-memory store of task records. Cheap to clone (one
/// `Arc` bump) so it can be passed into `tokio::spawn` or handed to
/// axum state alongside Tauri state.
#[derive(Clone, Default)]
pub struct TaskRegistry {
    inner: Arc<RwLock<HashMap<String, TaskRecord>>>,
    /// Monotonic counter handed out on every `start()` call. The
    /// spawned task captures this and supplies it to `complete()` /
    /// `fail()`; the registry only accepts terminal updates whose
    /// generation matches the slot's current generation. That's how
    /// we avoid a dismiss + restart race where the old task finishes
    /// after the new one was inserted and then clobbers its status.
    next_generation: Arc<AtomicU64>,
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
        let id = kind.id();
        let mut guard = self.inner.write().await;
        if let Some(existing) = guard.get(&id)
            && !existing.status.is_terminal()
        {
            return Err(format!("task already running: {id}"));
        }
        let generation = self.next_generation.fetch_add(1, Ordering::SeqCst);
        let now = Utc::now();
        let record = TaskRecord {
            id: id.clone(),
            kind,
            status: TaskStatus::Running,
            started_at: now,
            updated_at: now,
            generation,
        };
        guard.insert(id, record.clone());
        Ok(record)
    }

    /// Mark a task completed. `generation` must match the slot's
    /// current generation — stale updates from a dismissed run are
    /// silently dropped.
    pub async fn complete(
        &self,
        id: &str,
        generation: u64,
        summary: String,
        output_path: Option<String>,
    ) {
        let mut guard = self.inner.write().await;
        if let Some(rec) = guard.get_mut(id)
            && rec.generation == generation
        {
            rec.status = TaskStatus::Completed {
                summary,
                output_path,
            };
            rec.updated_at = Utc::now();
        }
    }

    /// Mark a task failed. Same generation check as `complete()`.
    pub async fn fail(&self, id: &str, generation: u64, error: String) {
        let mut guard = self.inner.write().await;
        if let Some(rec) = guard.get_mut(id)
            && rec.generation == generation
        {
            rec.status = TaskStatus::Failed { error };
            rec.updated_at = Utc::now();
        }
    }

    /// Explicit "user clicked dismiss" — remove the entry even if
    /// it's still running. The background tokio task keeps going
    /// and its final update lands in an empty slot (dropped) or,
    /// after a restart, in a *new* slot with a different generation
    /// (also dropped). Either way the new task's status stays intact.
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

    fn sample_video_kind(srt: &str) -> TaskKind {
        TaskKind::SubtitleAlignToVideo {
            srt_path: srt.to_string(),
            video_path: "/tmp/video.mkv".to_string(),
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
    async fn different_kinds_on_same_srt_are_independent() {
        let reg = TaskRegistry::new();
        reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        // A video-align on the same srt should get its own slot.
        reg.start(sample_video_kind("/tmp/a.srt")).await.unwrap();
        assert_eq!(reg.list().await.len(), 2);
    }

    #[tokio::test]
    async fn duplicate_start_allowed_after_terminal() {
        let reg = TaskRegistry::new();
        let rec = reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        reg.complete(&rec.id, rec.generation, "done".into(), None)
            .await;
        // Completed tasks don't block a restart
        reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        let rec = reg.get(&sample_kind("/tmp/a.srt").id()).await.unwrap();
        assert!(matches!(rec.status, TaskStatus::Running));
    }

    #[tokio::test]
    async fn complete_updates_status() {
        let reg = TaskRegistry::new();
        let rec = reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        reg.complete(
            &rec.id,
            rec.generation,
            "aligned 42 cues".into(),
            Some("/tmp/a.aligned.srt".into()),
        )
        .await;
        let rec2 = reg.get(&rec.id).await.unwrap();
        match rec2.status {
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
        let rec = reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        reg.fail(&rec.id, rec.generation, "ffmpeg exploded".into())
            .await;
        let rec2 = reg.get(&rec.id).await.unwrap();
        assert!(matches!(rec2.status, TaskStatus::Failed { .. }));
    }

    #[tokio::test]
    async fn dismiss_removes_entry() {
        let reg = TaskRegistry::new();
        let rec = reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        assert!(reg.dismiss(&rec.id).await);
        assert!(reg.get(&rec.id).await.is_none());
        // Dismissing a nonexistent id returns false
        assert!(!reg.dismiss("/tmp/nope.srt").await);
    }

    #[tokio::test]
    async fn stale_complete_after_dismiss_restart_is_dropped() {
        let reg = TaskRegistry::new();
        // First start + dismiss.
        let rec_old = reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        reg.dismiss(&rec_old.id).await;
        // Second start — different generation.
        let rec_new = reg.start(sample_kind("/tmp/a.srt")).await.unwrap();
        assert_ne!(rec_old.generation, rec_new.generation);
        // Now the old task's spawned future finally finishes and
        // tries to record its result — must NOT overwrite the new run.
        reg.complete(
            &rec_old.id,
            rec_old.generation,
            "done but stale".into(),
            None,
        )
        .await;
        let current = reg.get(&rec_new.id).await.unwrap();
        assert!(
            matches!(current.status, TaskStatus::Running),
            "new run must stay Running, got {:?}",
            current.status
        );
    }
}
