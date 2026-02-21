use blowup::sub::{align, fetch, shift};
use blowup::{config, config_cmd, download, omdb, search, tracker};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "blowup", about = "中文观影自动化流水线工具")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "搜索电影片源（优先 YIFY）")]
    Search {
        query: String,
        #[arg(long)]
        year: Option<u32>,
    },
    #[command(about = "通过 aria2c 下载种子/magnet")]
    Download {
        target: String,
        #[arg(long, default_value = ".")]
        output_dir: PathBuf,
    },
    #[command(subcommand, about = "字幕相关工具")]
    Sub(SubCommands),
    #[command(subcommand, about = "Tracker 列表管理")]
    Tracker(TrackerCommands),
    #[command(about = "通过 OMDB API 查询电影信息")]
    Info {
        query: String,
        #[arg(long)]
        year: Option<u32>,
    },
    #[command(subcommand, about = "管理 blowup 配置")]
    Config(ConfigCommands),
}

#[derive(Subcommand)]
enum SubCommands {
    #[command(about = "从 Assrt/OpenSubtitles 下载字幕")]
    Fetch {
        video: PathBuf,
        #[arg(long, default_value = "zh")]
        lang: String,
    },
    #[command(about = "用 alass 自动对齐字幕")]
    Align { video: PathBuf, srt: PathBuf },
    #[command(about = "从视频容器提取内嵌字幕流")]
    Extract {
        video: PathBuf,
        #[arg(long)]
        stream: Option<u32>,
    },
    #[command(about = "列出视频中的字幕流")]
    List { video: PathBuf },
    #[command(about = "手动偏移字幕时间戳（毫秒）")]
    Shift { srt: PathBuf, offset_ms: i64 },
}

#[derive(Subcommand)]
enum ConfigCommands {
    #[command(about = "设置配置项 (格式: section.field value)")]
    Set { key: String, value: String },
    #[command(about = "读取配置项 (格式: section.field)")]
    Get { key: String },
    #[command(about = "列出所有配置项")]
    List,
}

#[derive(Subcommand)]
enum TrackerCommands {
    #[command(about = "从远程源更新本地 tracker 列表")]
    Update {
        #[arg(long)]
        source: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cfg = config::load_config();

    match cli.command {
        Commands::Search { query, year } => {
            let results = search::search_yify(&query, year).await?;
            for (i, r) in results.iter().enumerate() {
                println!(
                    "{}: {} ({}) [{}] seeds={}",
                    i + 1,
                    r.title,
                    r.year,
                    r.quality,
                    r.seeds
                );
                if let Some(m) = &r.magnet {
                    println!("   magnet: {}", m);
                }
                if let Some(u) = &r.torrent_url {
                    println!("   torrent: {}", u);
                }
            }
        }
        Commands::Download { target, output_dir } => {
            download::download(download::DownloadArgs {
                target: &target,
                output_dir: &output_dir,
                aria2c_bin: &cfg.tools.aria2c,
            })
            .await?;
        }
        Commands::Sub(sub_cmd) => match sub_cmd {
            SubCommands::Fetch { video, lang } => {
                fetch::fetch_subtitle(&video, &lang, fetch::SubSource::All, &cfg).await?;
            }
            SubCommands::Align { video, srt } => {
                align::align_subtitle(&video, &srt)?;
            }
            SubCommands::Extract { video, stream } => {
                blowup::sub::extract_sub_srt(&video, stream).await?;
            }
            SubCommands::List { video } => {
                blowup::sub::list_all_subtitle_stream(&video).await?;
            }
            SubCommands::Shift { srt, offset_ms } => {
                shift::shift_srt(&srt, offset_ms)?;
            }
        },
        Commands::Tracker(TrackerCommands::Update { source }) => {
            tracker::update_tracker_list(source).await?;
        }
        Commands::Info { query, year } => {
            let api_key = &cfg.omdb.api_key;
            let movie = omdb::query_omdb(api_key, &query, year).await?;
            movie.print_info();
        }
        Commands::Config(config_cmd_args) => match config_cmd_args {
            ConfigCommands::Set { key, value } => {
                config_cmd::set_config(&key, &value)?;
            }
            ConfigCommands::Get { key } => {
                config_cmd::get_config(&key)?;
            }
            ConfigCommands::List => {
                config_cmd::list_config()?;
            }
        },
    }
    Ok(())
}
