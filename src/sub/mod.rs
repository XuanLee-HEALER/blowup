//! sub module
//!
//! 关于字幕处理的一些方法

pub mod align;
pub mod fetch;
pub mod shift;

use serde::Deserialize;
use serde::Serialize;

use std::path::Path;

use crate::ffmpeg::{FfmpegError, FfmpegTool};

/// 将 file 视频容器中的指定字幕流以 srt 文件格式提取到 sub 路径中。
/// stream 为 None 时提取第一个字幕流（0:s:0）。
pub async fn extract_sub_srt(
    file: impl AsRef<Path>,
    stream: Option<u32>,
) -> Result<(), FfmpegError> {
    let stream_idx = stream.unwrap_or(0);
    let map_spec = format!("0:s:{}", stream_idx);
    let file_str = file.as_ref().to_str().unwrap_or("");
    // 输出到同目录下的 .srt 文件
    let out = file
        .as_ref()
        .with_extension("srt")
        .to_str()
        .unwrap_or("")
        .to_string();
    let options = vec![
        "-i", file_str, "-map", &map_spec, "-c", "copy", &out,
    ];
    FfmpegTool::Ffmpeg
        .exec_with_options(None::<&'static str>, Some(options))
        .await?;
    Ok(())
}

/// 视频流的顶层结构体，用于解析 ffprobe 的 JSON 输出。
#[derive(Debug, Deserialize, Serialize)]
struct FfprobeOutput {
    streams: Vec<FfprobeStream>,
}

/// 单个流的详细信息
#[derive(Debug, Deserialize, Serialize)]
struct FfprobeStream {
    index: u32,
    codec_type: String,
    codec_name: String,
    start_time: String,
    duration_ts: u32,
    tags: Option<FfprobeTags>,
}

/// 流的标签信息
#[derive(Debug, Deserialize, Serialize)]
struct FfprobeTags {
    language: Option<String>,
    title: Option<String>,
}

/// 最终返回给调用者的字幕流信息结构体
#[derive(Debug, Clone, Serialize)]
pub struct SubtitleStreamInfo {
    pub index: u32,
    pub codec_name: String,
    pub duration: u32,
    pub language: Option<String>,
    pub title: Option<String>,
}

/// 列出视频文件中所有的字幕流信息并打印（列表格式）。
pub async fn list_all_subtitle_stream(
    file: impl AsRef<Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = file.as_ref();
    if !file_path.exists() {
        return Err(format!("文件不存在: {}", file_path.display()).into());
    }

    let mut args: Vec<String> = vec![
        "-v".to_string(),
        "quiet".to_string(),
        "-print_format".to_string(),
        "json".to_string(),
        "-show_streams".to_string(),
        "-select_streams".to_string(),
        "s".to_string(),
        "--".to_string(),
    ];
    args.push(file_path.to_string_lossy().to_string());

    let (stdout, _) = FfmpegTool::Ffprobe
        .exec_with_options(None::<&'static str>, Some(args))
        .await?;

    if stdout.is_empty() {
        println!("未找到任何字幕流。");
        return Ok(());
    }

    let output: FfprobeOutput = serde_json::from_str(&stdout)?;
    for stream in output.streams {
        let language = stream
            .tags
            .as_ref()
            .and_then(|t| t.language.as_deref())
            .unwrap_or("N/A");
        let title = stream
            .tags
            .as_ref()
            .and_then(|t| t.title.as_deref())
            .unwrap_or("N/A");
        println!(
            "Index({}) Codec({}) Duration({}ms) Language({}) Title({})",
            stream.index, stream.codec_name, stream.duration_ts, language, title
        );
    }

    Ok(())
}
