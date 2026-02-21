//! sub module
//!
//! 关于字幕处理的一些方法

use clap::ValueEnum;
use serde::Deserialize;
use serde::Serialize;

use std::path::Path;

use crate::ffmpeg::{FfmpegError, FfmpegTool};

/// 将 file 视频容器中的字幕流以srt文件的格式提取到 sub 路径中
pub async fn extract_sub_srt<P: AsRef<Path>>(file: P, sub: P) -> Result<(), FfmpegError> {
    let options = vec![
        "-i",
        file.as_ref().to_str().unwrap_or(""),
        "-map",
        "0:s:0",
        "-c",
        "copy",
        sub.as_ref().to_str().unwrap_or(""),
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

/// 定义输出格式的枚举类型。
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Json,
    #[clap(name = "tab")]
    Table,
    List,
}

/// 列出视频文件中所有的字幕流信息并直接打印。
pub async fn list_all_subtitle_stream(
    file: impl AsRef<Path>,
    format: OutputFormat,
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
    let subtitle_streams: Vec<SubtitleStreamInfo> = output
        .streams
        .into_iter()
        .map(|stream| SubtitleStreamInfo {
            index: stream.index,
            codec_name: stream.codec_name,
            language: stream.tags.as_ref().and_then(|tags| tags.language.clone()),
            title: stream.tags.as_ref().and_then(|tags| tags.title.clone()),
            duration: stream.duration_ts,
        })
        .collect();

    match format {
        OutputFormat::Json => {
            let json_output = serde_json::to_string_pretty(&subtitle_streams)?;
            println!("{}", json_output);
        }
        OutputFormat::Table => {
            for stream in subtitle_streams {
                println!(
                    "Index({}) Codec({}) Duration({}ms) Language({}) Title({})",
                    stream.index,
                    stream.codec_name,
                    stream.duration,
                    stream.language.unwrap_or_else(|| "N/A".to_string()),
                    stream.title.unwrap_or_else(|| "N/A".to_string())
                );
            }
        }
        OutputFormat::List => {
            for stream in subtitle_streams {
                println!(
                    "Index({}) Codec Name({}) Duration({}ms) Language({}) Title({})",
                    stream.index,
                    stream.codec_name,
                    stream.duration,
                    stream.language.unwrap_or_else(|| "N/A".to_string()),
                    stream.title.unwrap_or_else(|| "N/A".to_string())
                );
            }
        }
    }

    Ok(())
}
