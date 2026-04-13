//! Fire-and-forget wrappers around long-running service functions.
//!
//! Each `run_*` helper:
//!
//!   1. Inserts a Running record into `TaskRegistry`.
//!   2. Publishes `DomainEvent::TasksChanged` so subscribers (Tauri
//!      event forwarder, server SSE endpoint) can react immediately.
//!   3. `tokio::spawn`s the real work so the caller's IPC/HTTP
//!      future returns right away with the task id.
//!   4. In the spawned task, awaits the result, updates the registry
//!      to Completed / Failed (guarded by the task's generation so a
//!      dismissed + restarted slot doesn't get clobbered), and
//!      publishes another TasksChanged.

use crate::infra::events::{DomainEvent, EventBus};
use crate::subtitle::service as sub;
use crate::tasks::model::TaskKind;
use crate::tasks::registry::TaskRegistry;
use std::path::PathBuf;

/// Start a subtitle-to-audio alignment in the background. Returns the
/// task id on success; fails fast if a task is already running for
/// the same SRT.
pub async fn run_subtitle_align_to_audio(
    registry: TaskRegistry,
    events: EventBus,
    srt: PathBuf,
    audio: PathBuf,
) -> Result<String, String> {
    let kind = TaskKind::SubtitleAlignToAudio {
        srt_path: srt.to_string_lossy().into_owned(),
        audio_path: audio.to_string_lossy().into_owned(),
    };
    let record = registry.start(kind).await?;
    events.publish(DomainEvent::TasksChanged);
    let id = record.id.clone();
    let generation = record.generation;

    let reg = registry.clone();
    let evts = events.clone();
    let id_for_task = id.clone();
    tokio::spawn(async move {
        tracing::info!(task_id = %id_for_task, "subtitle align-to-audio started");
        let result = sub::align_subtitle_to_audio(&srt, &audio).await;
        match result {
            Ok(r) => {
                tracing::info!(task_id = %id_for_task, "subtitle align-to-audio completed");
                reg.complete(&id_for_task, generation, r.summary, Some(r.output_path))
                    .await;
            }
            Err(e) => {
                tracing::warn!(task_id = %id_for_task, error = %e, "subtitle align-to-audio failed");
                reg.fail(&id_for_task, generation, e.to_string()).await;
            }
        }
        evts.publish(DomainEvent::TasksChanged);
    });

    Ok(id)
}

/// Start a subtitle-to-video alignment (extracts audio internally).
pub async fn run_subtitle_align_to_video(
    registry: TaskRegistry,
    events: EventBus,
    srt: PathBuf,
    video: PathBuf,
) -> Result<String, String> {
    let kind = TaskKind::SubtitleAlignToVideo {
        srt_path: srt.to_string_lossy().into_owned(),
        video_path: video.to_string_lossy().into_owned(),
    };
    let record = registry.start(kind).await?;
    events.publish(DomainEvent::TasksChanged);
    let id = record.id.clone();
    let generation = record.generation;

    let reg = registry.clone();
    let evts = events.clone();
    let id_for_task = id.clone();
    tokio::spawn(async move {
        tracing::info!(task_id = %id_for_task, "subtitle align-to-video started");
        let result = sub::align_subtitle(&video, &srt).await;
        match result {
            Ok(()) => {
                tracing::info!(task_id = %id_for_task, "subtitle align-to-video completed");
                // align_subtitle overwrites the original SRT in place — no separate output path
                reg.complete(
                    &id_for_task,
                    generation,
                    "对齐完成（原文件已更新）".to_string(),
                    None,
                )
                .await;
            }
            Err(e) => {
                tracing::warn!(task_id = %id_for_task, error = %e, "subtitle align-to-video failed");
                reg.fail(&id_for_task, generation, e.to_string()).await;
            }
        }
        evts.publish(DomainEvent::TasksChanged);
    });

    Ok(id)
}
