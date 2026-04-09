use regex::Regex;
use serde::Deserialize;
use std::path::Path;
use std::sync::LazyLock;

// ── SRT parsing ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SubCue {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

static TIMESTAMP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d{2}):(\d{2}):(\d{2})[,.](\d{3})\s*-->\s*(\d{2}):(\d{2}):(\d{2})[,.](\d{3})")
        .expect("valid SRT timestamp regex")
});

fn parse_ts(caps: &regex::Captures, offset: usize) -> i64 {
    let h: i64 = caps[offset].parse().unwrap_or(0);
    let m: i64 = caps[offset + 1].parse().unwrap_or(0);
    let s: i64 = caps[offset + 2].parse().unwrap_or(0);
    let ms: i64 = caps[offset + 3].parse().unwrap_or(0);
    h * 3_600_000 + m * 60_000 + s * 1_000 + ms
}

pub fn parse_srt(content: &str) -> Vec<SubCue> {
    // Strip BOM
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let content = content.replace('\r', "");

    let mut cues = Vec::new();
    let mut lines = content.split('\n').peekable();

    while let Some(line) = lines.next() {
        // Look for timestamp lines; skip everything else (blank lines, sequence numbers, garbage)
        let caps = match TIMESTAMP_RE.captures(line) {
            Some(c) => c,
            None => continue,
        };
        let start_ms = parse_ts(&caps, 1);
        let end_ms = parse_ts(&caps, 5);

        // Collect text lines until blank line or end
        let mut text_parts = Vec::new();
        while let Some(line) = lines.peek() {
            if line.trim().is_empty() {
                lines.next();
                break;
            }
            text_parts.push(lines.next().unwrap().to_string());
        }

        if !text_parts.is_empty() {
            cues.push(SubCue {
                start_ms,
                end_ms,
                text: text_parts.join("\\N"), // ASS line break
            });
        }
    }

    cues.sort_by_key(|c| c.start_ms);
    cues
}

// ── ASS generation ──────────────────────────────────────────────

const PLAY_RES_Y: f64 = 1080.0;

#[derive(Debug, Clone, Deserialize)]
pub struct SubtitleOverlayConfig {
    pub path: String,
    /// 0.0 = bottom edge, 1.0 = top edge
    pub y_position: f64,
    /// Hex color "#RRGGBB"
    pub color: String,
    pub font_size: u32,
}

/// Convert "#RRGGBB" to ASS "&H00BBGGRR" format.
fn hex_to_ass_color(hex: &str) -> String {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return "&H00FFFFFF".to_string();
    }
    let r = &hex[0..2];
    let g = &hex[2..4];
    let b = &hex[4..6];
    format!("&H00{b}{g}{r}").to_uppercase()
}

/// Map y_position (0.0=bottom, 1.0=top) to ASS Alignment + MarginV.
fn y_to_alignment_margin(y: f64) -> (u8, u32) {
    let y = y.clamp(0.0, 1.0);
    if y < 0.5 {
        // Bottom half: Alignment=2, MarginV grows as y→0
        let margin = ((0.5 - y) * 2.0 * PLAY_RES_Y * 0.4) as u32;
        (2, margin)
    } else {
        // Top half: Alignment=8, MarginV grows as y→1
        let margin = ((y - 0.5) * 2.0 * PLAY_RES_Y * 0.4) as u32;
        (8, margin)
    }
}

fn format_ass_ts(ms: i64) -> String {
    let ms = ms.max(0);
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let cs = (ms % 1_000) / 10;
    format!("{h}:{m:02}:{s:02}.{cs:02}")
}

/// Format milliseconds as SRT timestamp: `HH:MM:SS,mmm`
pub fn format_srt_ts(ms: i64) -> String {
    let ms = ms.max(0);
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;
    format!("{h:02}:{m:02}:{s:02},{millis:03}")
}

pub fn merge_to_ass(configs: &[SubtitleOverlayConfig]) -> Result<String, String> {
    let mut styles = String::new();
    let mut events = String::new();

    for (i, cfg) in configs.iter().enumerate() {
        let content = std::fs::read_to_string(&cfg.path)
            .map_err(|e| format!("读取字幕文件失败 {}: {}", cfg.path, e))?;
        let cues = parse_srt(&content);
        let (alignment, margin_v) = y_to_alignment_margin(cfg.y_position);
        let ass_color = hex_to_ass_color(&cfg.color);
        let style_name = format!("sub{i}");

        // Style line
        styles.push_str(&format!(
            "Style: {style_name},Sans,{},{ass_color},&H000000FF,&H00000000,&H80000000,\
             0,0,0,0,100,100,0,0,1,2,1,{alignment},20,20,{margin_v},1\n",
            cfg.font_size,
        ));

        // Dialogue lines
        for cue in &cues {
            events.push_str(&format!(
                "Dialogue: 0,{},{},{style_name},,0,0,0,,{}\n",
                format_ass_ts(cue.start_ms),
                format_ass_ts(cue.end_ms),
                cue.text,
            ));
        }
    }

    Ok(format!(
        "[Script Info]\n\
         ScriptType: v4.00+\n\
         PlayResX: 1920\n\
         PlayResY: 1080\n\
         \n\
         [V4+ Styles]\n\
         Format: Name,Fontname,Fontsize,PrimaryColour,SecondaryColour,OutlineColour,BackColour,\
         Bold,Italic,Underline,StrikeOut,ScaleX,ScaleY,Spacing,Angle,BorderStyle,Outline,Shadow,\
         Alignment,MarginL,MarginR,MarginV,Encoding\n\
         {styles}\n\
         [Events]\n\
         Format: Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect,Text\n\
         {events}"
    ))
}

// ── Cache key ───────────────────────────────────────────────────

pub fn overlay_cache_key(configs: &[SubtitleOverlayConfig]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let mut sorted: Vec<_> = configs.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    for cfg in sorted {
        hasher.update(cfg.path.as_bytes());
        hasher.update(cfg.y_position.to_bits().to_le_bytes());
        hasher.update(cfg.color.as_bytes());
        hasher.update(cfg.font_size.to_le_bytes());
    }
    let hash = hasher.finalize();
    format!(".blowup_overlay_{}.ass", hex::encode(&hash[..8]))
}

/// Clean up stale overlay ASS files whose source SRTs no longer all exist.
pub fn cleanup_stale_overlays(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(".blowup_overlay_") && name.ends_with(".ass") {
            // Can't verify which SRTs contributed (hash is one-way),
            // so just remove all overlay caches — they'll be regenerated on next play.
            // This is called only when an SRT is deleted, so the cost is minimal.
            std::fs::remove_file(entry.path()).ok();
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_srt() {
        let srt = "1\n00:00:01,000 --> 00:00:03,500\nHello world\n\n2\n00:00:05,000 --> 00:00:08,000\nSecond line\n";
        let cues = parse_srt(srt);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start_ms, 1000);
        assert_eq!(cues[0].end_ms, 3500);
        assert_eq!(cues[0].text, "Hello world");
        assert_eq!(cues[1].start_ms, 5000);
    }

    #[test]
    fn parse_multiline_cue() {
        let srt = "1\n00:00:01,000 --> 00:00:03,000\nLine one\nLine two\n\n";
        let cues = parse_srt(srt);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "Line one\\NLine two");
    }

    #[test]
    fn parse_with_bom() {
        let srt = "\u{feff}1\n00:00:01,000 --> 00:00:02,000\nBOM test\n\n";
        let cues = parse_srt(srt);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "BOM test");
    }

    #[test]
    fn parse_crlf() {
        let srt = "1\r\n00:00:01,000 --> 00:00:02,000\r\nCRLF\r\n\r\n";
        let cues = parse_srt(srt);
        assert_eq!(cues.len(), 1);
    }

    #[test]
    fn hex_to_ass() {
        assert_eq!(hex_to_ass_color("#FF0000"), "&H000000FF"); // red
        assert_eq!(hex_to_ass_color("#00FF00"), "&H0000FF00"); // green
        assert_eq!(hex_to_ass_color("#FFFFFF"), "&H00FFFFFF"); // white
    }

    #[test]
    fn y_position_bottom() {
        let (align, margin) = y_to_alignment_margin(0.0);
        assert_eq!(align, 2);
        assert!(margin > 0);
    }

    #[test]
    fn y_position_top() {
        let (align, margin) = y_to_alignment_margin(1.0);
        assert_eq!(align, 8);
        assert!(margin > 0);
    }

    #[test]
    fn y_position_center_bottom() {
        let (align, _) = y_to_alignment_margin(0.49);
        assert_eq!(align, 2);
    }

    #[test]
    fn y_position_center_top() {
        let (align, _) = y_to_alignment_margin(0.5);
        assert_eq!(align, 8);
    }

    #[test]
    fn format_timestamp() {
        assert_eq!(format_ass_ts(0), "0:00:00.00");
        assert_eq!(format_ass_ts(3_723_450), "1:02:03.45");
    }

    #[test]
    fn cache_key_stable() {
        let configs = vec![
            SubtitleOverlayConfig {
                path: "/a.srt".into(),
                y_position: 0.1,
                color: "#FFFFFF".into(),
                font_size: 48,
            },
            SubtitleOverlayConfig {
                path: "/b.srt".into(),
                y_position: 0.9,
                color: "#FFFF00".into(),
                font_size: 36,
            },
        ];
        let key1 = overlay_cache_key(&configs);
        let key2 = overlay_cache_key(&configs);
        assert_eq!(key1, key2);
        assert!(key1.starts_with(".blowup_overlay_"));
        assert!(key1.ends_with(".ass"));
    }

    #[test]
    fn cache_key_order_independent() {
        let a = SubtitleOverlayConfig {
            path: "/a.srt".into(),
            y_position: 0.1,
            color: "#FFFFFF".into(),
            font_size: 48,
        };
        let b = SubtitleOverlayConfig {
            path: "/b.srt".into(),
            y_position: 0.9,
            color: "#FFFF00".into(),
            font_size: 36,
        };
        let key1 = overlay_cache_key(&[a.clone(), b.clone()]);
        let key2 = overlay_cache_key(&[b, a]);
        assert_eq!(key1, key2);
    }
}
