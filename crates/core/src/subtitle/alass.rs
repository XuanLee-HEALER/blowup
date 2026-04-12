//! Subtitle alignment using alass-core library + WebRTC VAD.
//!
//! Replaces the external alass CLI binary. Pipeline:
//! 1. ffmpeg extracts 8 kHz mono PCM from reference media
//! 2. WebRTC VAD detects voice segments → reference TimeSpans
//! 3. SRT cues are converted to subtitle TimeSpans
//! 4. alass-core aligns the two TimeSpan lists
//! 5. Per-cue deltas are applied to produce corrected SRT

use std::io::Cursor;
use std::path::Path;

use alass_core::{self, NoProgressHandler, TimePoint, TimeSpan};
use byteorder::{LittleEndian, ReadBytesExt};
use webrtc_vad::Vad;

use crate::infra::ffmpeg::FfmpegTool;
use crate::subtitle::parser::SubCue;

// ── Constants ────────────────────────────────────────────────────

const SAMPLE_RATE: u32 = 8000;
/// 10 ms frame at 8 kHz = 80 samples
const FRAME_SAMPLES: usize = 80;
/// Discard voice segments shorter than this
const MIN_SEGMENT_MS: i64 = 500;
/// alass default split penalty
const SPLIT_PENALTY: f64 = 7.0;
/// Speed optimization: 1.0 matches alass-cli default (good balance of speed vs accuracy)
const SPEED_OPTIMIZATION: Option<f64> = Some(1.0);

// ── PCM extraction ───────────────────────────────────────────────

/// Use ffmpeg to extract audio as 8 kHz mono 16-bit LE PCM into a temp file,
/// then read the raw samples.
pub(crate) async fn extract_pcm(media: &Path) -> Result<Vec<i16>, String> {
    let pid = std::process::id();
    let tmp = media.with_extension(format!("_alass_{pid}.raw"));
    let media_str = media.to_string_lossy().to_string();
    let tmp_str = tmp.to_string_lossy().to_string();

    let options = vec![
        "-i".to_string(),
        media_str,
        "-ar".to_string(),
        SAMPLE_RATE.to_string(),
        "-ac".to_string(),
        "1".to_string(),
        "-f".to_string(),
        "s16le".to_string(),
        "-y".to_string(),
        tmp_str.clone(),
    ];

    FfmpegTool::Ffmpeg
        .exec_with_options(None::<&'static str>, Some(options))
        .await
        .map_err(|e| format!("ffmpeg PCM 提取失败: {e}"))?;

    let bytes = std::fs::read(&tmp).map_err(|e| format!("读取 PCM 临时文件失败: {e}"))?;
    std::fs::remove_file(&tmp).ok();

    let mut cursor = Cursor::new(bytes);
    let mut samples = Vec::new();
    while let Ok(s) = cursor.read_i16::<LittleEndian>() {
        samples.push(s);
    }

    if samples.is_empty() {
        return Err("ffmpeg 未产生音频数据".into());
    }
    Ok(samples)
}

// ── VAD ──────────────────────────────────────────────────────────

/// Run WebRTC VAD on PCM samples, returning voice-activity TimeSpans (in ms).
pub(crate) fn pcm_to_voice_spans(samples: &[i16]) -> Vec<TimeSpan> {
    let mut vad = Vad::new();
    vad.set_mode(webrtc_vad::VadMode::Quality);

    let mut segments = Vec::new();
    let mut in_voice = false;
    let mut seg_start_ms: i64 = 0;

    for (i, chunk) in samples.chunks(FRAME_SAMPLES).enumerate() {
        if chunk.len() < FRAME_SAMPLES {
            break;
        }
        let frame_ms = (i as i64) * 10;
        let is_voice = vad.is_voice_segment(chunk).unwrap_or(false);

        if is_voice && !in_voice {
            seg_start_ms = frame_ms;
            in_voice = true;
        } else if !is_voice && in_voice {
            if frame_ms - seg_start_ms >= MIN_SEGMENT_MS {
                segments.push(TimeSpan::new(
                    TimePoint::from(seg_start_ms),
                    TimePoint::from(frame_ms),
                ));
            }
            in_voice = false;
        }
    }

    // Close trailing segment
    if in_voice {
        let end_ms = (samples.len() / FRAME_SAMPLES) as i64 * 10;
        if end_ms - seg_start_ms >= MIN_SEGMENT_MS {
            segments.push(TimeSpan::new(
                TimePoint::from(seg_start_ms),
                TimePoint::from(end_ms),
            ));
        }
    }

    segments
}

// ── Alignment ────────────────────────────────────────────────────

fn cues_to_timespans(cues: &[SubCue]) -> Vec<TimeSpan> {
    cues.iter()
        .map(|c| TimeSpan::new(TimePoint::from(c.start_ms), TimePoint::from(c.end_ms)))
        .collect()
}

/// Result of alignment: corrected cues + summary info.
pub struct AlignOutput {
    pub cues: Vec<SubCue>,
    pub summary: String,
}

/// Align subtitle cues to a reference media file (video or audio).
pub async fn align_to_media(cues: &[SubCue], media: &Path) -> Result<AlignOutput, String> {
    if cues.is_empty() {
        return Err("字幕为空".into());
    }

    // Step 1: Extract audio as 8kHz mono PCM
    let t0 = std::time::Instant::now();
    let samples = extract_pcm(media).await?;
    tracing::info!(
        samples = samples.len(),
        elapsed_ms = t0.elapsed().as_millis(),
        "PCM extraction done"
    );

    // Step 2: VAD + alignment are CPU-bound — run off the async thread pool
    let cues = cues.to_vec();
    tokio::task::spawn_blocking(move || {
        // VAD: detect voice segments
        let t1 = std::time::Instant::now();
        let ref_spans = pcm_to_voice_spans(&samples);
        tracing::info!(
            segments = ref_spans.len(),
            elapsed_ms = t1.elapsed().as_millis(),
            "VAD done"
        );
        if ref_spans.is_empty() {
            return Err("未在音频中检测到语音活动".into());
        }

        let sub_spans = cues_to_timespans(&cues);
        tracing::info!(
            ref_spans = ref_spans.len(),
            sub_spans = sub_spans.len(),
            "starting alass alignment"
        );

        // Alignment
        let t2 = std::time::Instant::now();
        let (deltas, _score) = alass_core::align(
            &ref_spans,
            &sub_spans,
            SPLIT_PENALTY,
            SPEED_OPTIMIZATION,
            alass_core::standard_scoring,
            NoProgressHandler,
        );
        tracing::info!(
            elapsed_ms = t2.elapsed().as_millis(),
            "alass alignment done"
        );

        // Build summary from deltas
        let summary = summarize_deltas(&deltas);

        // Apply per-cue deltas
        let aligned = cues
            .iter()
            .zip(deltas.iter())
            .map(|(cue, delta)| {
                let offset_ms = delta.as_i64();
                SubCue {
                    start_ms: (cue.start_ms + offset_ms).max(0),
                    end_ms: (cue.end_ms + offset_ms).max(0),
                    text: cue.text.clone(),
                }
            })
            .collect();

        Ok(AlignOutput {
            cues: aligned,
            summary,
        })
    })
    .await
    .map_err(|e| format!("alignment task panicked: {e}"))?
}

/// Generate a human-readable summary from per-cue deltas.
/// Groups consecutive cues with the same offset into blocks.
fn summarize_deltas(deltas: &[alass_core::TimeDelta]) -> String {
    if deltas.is_empty() {
        return "无字幕".into();
    }

    // Group consecutive cues with same delta into blocks
    let mut blocks: Vec<(i64, usize)> = Vec::new(); // (offset_ms, count)
    for d in deltas {
        let ms = d.as_i64();
        if let Some(last) = blocks.last_mut()
            && last.0 == ms
        {
            last.1 += 1;
            continue;
        }
        blocks.push((ms, 1));
    }

    if blocks.len() == 1 {
        // Uniform shift
        let (ms, count) = blocks[0];
        if ms == 0 {
            return format!("已对齐，{count} 条字幕无需调整");
        }
        return format!("统一偏移 {count} 条字幕 {}", format_offset_ms(ms));
    }

    // Multiple blocks — show the largest ones
    let total: usize = blocks.iter().map(|(_, c)| c).sum();
    let zero_count: usize = blocks
        .iter()
        .filter(|(ms, _)| *ms == 0)
        .map(|(_, c)| c)
        .sum();
    let shifted = total - zero_count;

    let mut parts = Vec::new();
    parts.push(format!("共 {total} 条字幕，{} 个分段", blocks.len()));
    if shifted > 0 {
        // Find the dominant non-zero offset
        let dominant = blocks
            .iter()
            .filter(|(ms, _)| *ms != 0)
            .max_by_key(|(_, c)| *c);
        if let Some((ms, count)) = dominant {
            parts.push(format!("主要偏移: {} ({count} 条)", format_offset_ms(*ms)));
        }
    }
    if zero_count > 0 {
        parts.push(format!("{zero_count} 条无需调整"));
    }

    parts.join("；")
}

fn format_offset_ms(ms: i64) -> String {
    let sign = if ms >= 0 { "+" } else { "-" };
    let abs = ms.unsigned_abs();
    let s = abs / 1000;
    let frac = abs % 1000;
    if s >= 60 {
        let m = s / 60;
        let s = s % 60;
        format!("{sign}{m}:{s:02}.{frac:03}")
    } else {
        format!("{sign}{s}.{frac:03}s")
    }
}

/// Format aligned cues back to SRT string.
pub fn cues_to_srt(cues: &[SubCue]) -> String {
    use crate::subtitle::parser::format_srt_ts;
    let mut out = String::new();
    for (i, cue) in cues.iter().enumerate() {
        out.push_str(&format!("{}\n", i + 1));
        out.push_str(&format!(
            "{} --> {}\n",
            format_srt_ts(cue.start_ms),
            format_srt_ts(cue.end_ms),
        ));
        // Convert ASS line breaks back to real newlines for SRT
        out.push_str(&cue.text.replace("\\N", "\n"));
        out.push_str("\n\n");
    }
    out
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitle::parser::format_srt_ts;

    #[test]
    fn vad_empty_input() {
        let spans = pcm_to_voice_spans(&[]);
        assert!(spans.is_empty());
    }

    #[test]
    fn vad_silence() {
        // 1 second of silence at 8 kHz
        let samples = vec![0i16; 8000];
        let spans = pcm_to_voice_spans(&samples);
        assert!(spans.is_empty());
    }

    #[test]
    fn cues_roundtrip_srt() {
        let cues = vec![
            SubCue {
                start_ms: 1000,
                end_ms: 3500,
                text: "Hello".into(),
            },
            SubCue {
                start_ms: 5000,
                end_ms: 8000,
                text: "Line 1\\NLine 2".into(),
            },
        ];
        let srt = cues_to_srt(&cues);
        assert!(srt.contains("00:00:01,000 --> 00:00:03,500"));
        assert!(srt.contains("00:00:05,000 --> 00:00:08,000"));
        assert!(srt.contains("Line 1\nLine 2"));
    }

    #[test]
    fn format_srt_timestamp() {
        assert_eq!(format_srt_ts(0), "00:00:00,000");
        assert_eq!(format_srt_ts(3_723_450), "01:02:03,450");
        assert_eq!(format_srt_ts(-100), "00:00:00,000");
    }

    #[test]
    fn cues_to_timespans_basic() {
        let cues = vec![SubCue {
            start_ms: 1000,
            end_ms: 2000,
            text: "test".into(),
        }];
        let spans = cues_to_timespans(&cues);
        assert_eq!(spans.len(), 1);
    }

    /// End-to-end alignment test with real files.
    /// Set BLOWUP_TEST_SRT and BLOWUP_TEST_AUDIO env vars to run.
    /// Example: BLOWUP_TEST_SRT="D:\...\foo.srt" BLOWUP_TEST_AUDIO="D:\...\foo.m4a" cargo test -p blowup alass_e2e -- --nocapture
    #[tokio::test]
    async fn alass_e2e() {
        let srt_path = match std::env::var("BLOWUP_TEST_SRT") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("BLOWUP_TEST_SRT not set, skipping e2e test");
                return;
            }
        };
        let audio_path = std::env::var("BLOWUP_TEST_AUDIO").expect("BLOWUP_TEST_AUDIO required");

        // 1. Parse SRT
        let t = std::time::Instant::now();
        let content = std::fs::read_to_string(&srt_path).expect("read SRT");
        let cues = crate::subtitle::parser::parse_srt(&content);
        eprintln!(
            "[1] SRT parsed: {} cues in {}ms",
            cues.len(),
            t.elapsed().as_millis()
        );
        assert!(!cues.is_empty(), "SRT should have cues");

        // 2. Extract PCM
        let t = std::time::Instant::now();
        let samples = extract_pcm(std::path::Path::new(&audio_path))
            .await
            .expect("extract PCM");
        eprintln!(
            "[2] PCM extracted: {} samples ({:.1}s audio) in {}ms",
            samples.len(),
            samples.len() as f64 / SAMPLE_RATE as f64,
            t.elapsed().as_millis()
        );
        assert!(!samples.is_empty());

        // 3. VAD
        let t = std::time::Instant::now();
        let ref_spans = pcm_to_voice_spans(&samples);
        eprintln!(
            "[3] VAD: {} voice segments in {}ms",
            ref_spans.len(),
            t.elapsed().as_millis()
        );
        assert!(!ref_spans.is_empty(), "should detect voice");

        // 4. Alignment
        let sub_spans = cues_to_timespans(&cues);
        eprintln!(
            "[4] Aligning {} ref x {} sub ...",
            ref_spans.len(),
            sub_spans.len()
        );
        let t = std::time::Instant::now();
        let (deltas, score) = alass_core::align(
            &ref_spans,
            &sub_spans,
            SPLIT_PENALTY,
            SPEED_OPTIMIZATION,
            alass_core::standard_scoring,
            NoProgressHandler,
        );
        eprintln!(
            "[4] Alignment done: score={score:.2} in {}ms",
            t.elapsed().as_millis()
        );
        assert_eq!(deltas.len(), cues.len());

        eprintln!("[OK] Full pipeline succeeded");
    }
}
