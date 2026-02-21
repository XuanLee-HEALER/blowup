use blowup::{
    sub::{OutputFormat, extract_sub_srt, list_all_subtitle_stream},
    tracker::update_tracker_list,
};
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(propagate_version = true, version = "0.1", about = "all about movie", long_about = None)]
struct Hermes {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "handle all things about tracker list")]
    Tracker(TrackerArgs),
    #[command(about = "subtitle file processing tools")]
    Sub(SubArgs),
}

#[derive(Args)]
struct TrackerArgs {
    #[command(subcommand)]
    commands: TrackerCommands,
}

#[derive(Subcommand)]
enum TrackerCommands {
    #[command(about = "update the newest tracker list")]
    Update {},
}

#[derive(Args)]
struct SubArgs {
    #[command(subcommand)]
    commands: SubCommands,
}

#[derive(Subcommand)]
enum SubCommands {
    #[command(name = "export", about = "Extract subtitle streams from the specified video container")]
    ExportSub {
        file_name: String,
        output_path: String,
    },
    #[command(name = "list", about = "List subtitle streams in a video container")]
    ListSubStream {
        file_name: String,
        #[arg(short = 'f', long = "format", help = "Output format: list/json/tab")]
        format: Option<OutputFormat>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Hermes::parse();

    match &cli.commands {
        Commands::Tracker(tracker_args) => match &tracker_args.commands {
            TrackerCommands::Update {} => update_tracker_list(None).await?,
        },
        Commands::Sub(sub_args) => match &sub_args.commands {
            SubCommands::ExportSub {
                file_name,
                output_path,
            } => extract_sub_srt(file_name, output_path)
                .await
                .expect("Failed to extract the subtitle stream from media file"),
            SubCommands::ListSubStream { file_name, format } => {
                list_all_subtitle_stream(file_name, format.unwrap_or(OutputFormat::List))
                    .await
                    .expect("Failed to retrieve subtitle stream information")
            }
        },
    }
    Ok(())
}
