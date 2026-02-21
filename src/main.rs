use blowup::{
    sub::{extract_sub_srt, list_all_subtitle_stream},
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
    #[command(name = "export", about = "Extract subtitle stream from video container")]
    ExportSub {
        file_name: String,
        #[arg(long)]
        stream: Option<u32>,
    },
    #[command(name = "list", about = "List subtitle streams in a video container")]
    ListSubStream {
        file_name: String,
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
            SubCommands::ExportSub { file_name, stream } => {
                extract_sub_srt(file_name, *stream).await?;
            }
            SubCommands::ListSubStream { file_name } => {
                list_all_subtitle_stream(file_name).await?;
            }
        },
    }
    Ok(())
}
