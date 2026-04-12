use chrono::{DateTime, Utc};
use serde::Serialize;

/// Structured description of what kind of work a task is doing.
/// The frontend uses the variant tag to decide which UI row to
/// hydrate; new long-running operations just add a new variant here.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskKind {
    /// Align a SRT subtitle to an audio track using alass-core.
    /// Task id = srt_path (one alignment per SRT at a time).
    SubtitleAlignToAudio {
        srt_path: String,
        audio_path: String,
    },
    /// Align a SRT subtitle to a video file (extracts audio first).
    /// Task id = srt_path.
    SubtitleAlignToVideo {
        srt_path: String,
        video_path: String,
    },
}

impl TaskKind {
    /// Unique key used as the HashMap index in `TaskRegistry`. Two
    /// tasks with the same key can't run concurrently — attempting
    /// to start a duplicate returns an error.
    pub fn id(&self) -> &str {
        match self {
            TaskKind::SubtitleAlignToAudio { srt_path, .. }
            | TaskKind::SubtitleAlignToVideo { srt_path, .. } => srt_path,
        }
    }
}

/// Lifecycle state of a task.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TaskStatus {
    Running,
    Completed {
        summary: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        output_path: Option<String>,
    },
    Failed {
        error: String,
    },
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Completed { .. } | TaskStatus::Failed { .. })
    }
}

/// A single task record held in `TaskRegistry`. Serialized to JSON
/// for both Tauri IPC responses and server REST responses.
#[derive(Debug, Clone, Serialize)]
pub struct TaskRecord {
    pub id: String,
    pub kind: TaskKind,
    pub status: TaskStatus,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
