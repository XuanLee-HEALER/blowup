use crate::error::SubError;
use std::fs;
use std::path::Path;

/// 将 SRT 文件中所有时间戳偏移 offset_ms 毫秒
pub fn shift_srt(srt_path: &Path, offset_ms: i64) -> Result<(), SubError> {
    let content = fs::read_to_string(srt_path).map_err(SubError::Io)?;
    let shifted = apply_offset(&content, offset_ms)?;
    fs::write(srt_path, shifted).map_err(SubError::Io)?;
    Ok(())
}

fn apply_offset(content: &str, offset_ms: i64) -> Result<String, SubError> {
    use regex::Regex;
    let re =
        Regex::new(r"(\d{2}):(\d{2}):(\d{2}),(\d{3}) --> (\d{2}):(\d{2}):(\d{2}),(\d{3})").unwrap();

    let result = re.replace_all(content, |caps: &regex::Captures| {
        let start = parse_ts(caps, 1) + offset_ms;
        let end = parse_ts(caps, 5) + offset_ms;
        format!("{} --> {}", format_ts(start.max(0)), format_ts(end.max(0)))
    });
    Ok(result.into_owned())
}

fn parse_ts(caps: &regex::Captures, offset: usize) -> i64 {
    let h: i64 = caps[offset].parse().unwrap_or(0);
    let m: i64 = caps[offset + 1].parse().unwrap_or(0);
    let s: i64 = caps[offset + 2].parse().unwrap_or(0);
    let ms: i64 = caps[offset + 3].parse().unwrap_or(0);
    h * 3_600_000 + m * 60_000 + s * 1_000 + ms
}

fn format_ts(total_ms: i64) -> String {
    let h = total_ms / 3_600_000;
    let m = (total_ms % 3_600_000) / 60_000;
    let s = (total_ms % 60_000) / 1_000;
    let ms = total_ms % 1_000;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_positive() {
        let srt = "1\n00:01:00,000 --> 00:01:05,000\nHello\n";
        let result = apply_offset(srt, 5000).unwrap();
        assert!(result.contains("00:01:05,000 --> 00:01:10,000"));
    }

    #[test]
    fn offset_negative() {
        let srt = "1\n00:01:00,000 --> 00:01:05,000\nHello\n";
        let result = apply_offset(srt, -10000).unwrap();
        assert!(result.contains("00:00:50,000 --> 00:00:55,000"));
    }

    #[test]
    fn clamp_at_zero() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nHello\n";
        let result = apply_offset(srt, -5000).unwrap();
        assert!(result.contains("00:00:00,000 --> 00:00:00,000"));
    }
}
