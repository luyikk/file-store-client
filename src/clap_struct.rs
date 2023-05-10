use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
pub enum Opt {
    /// create config
    Create,
    /// push file
    Push {
        /// save dir
        #[arg(long, short, value_parser)]
        dir: Option<PathBuf>,
        /// local file
        #[arg(value_parser)]
        file: PathBuf,
        /// async write
        #[arg(long, short, value_parser, default_value = "false")]
        r#async: bool,
        /// transfer block size default 131072
        #[arg(long, short, value_parser, default_value = "131072")]
        block: usize,
        /// if service exists file, over write file
        #[arg(long, short, value_parser, default_value = "false")]
        overwrite: bool,
    },
    /// pull file
    Pull {
        /// remote file path
        #[arg(value_parser)]
        file: PathBuf,
        /// save file path
        #[arg(long, short, value_parser)]
        save: Option<PathBuf>,
        /// transfer block size default 131072
        #[arg(long, short, value_parser, default_value = "131072")]
        block: usize,
        /// if exists file, over write file
        #[arg(long, short, value_parser, default_value = "false")]
        overwrite: bool,
    },
    /// image path
    Image(ImageArgs),
    /// show remote directory contents
    #[command(name = "show")]
    ShowDir {
        /// remote directory path
        #[arg(value_parser)]
        dir: PathBuf,
    },
    /// show remote file info
    Info {
        /// remote file path
        #[arg(value_parser)]
        file: PathBuf,
    },
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct ImageArgs {
    #[command(subcommand)]
    pub command: ImageCommands,
}

#[derive(Debug, Subcommand)]
pub enum ImageCommands {
    /// push image
    Push {
        /// save dir
        #[arg(long, short, value_parser)]
        dir: Option<PathBuf>,
        /// local path
        #[arg(value_parser)]
        path: PathBuf,
        /// async write
        #[arg(long, short, value_parser, default_value = "false")]
        r#async: bool,
        /// transfer block size default 131072
        #[arg(long, short, value_parser, default_value = "131072")]
        block: usize,
        /// if service exists file, over write file
        #[arg(long, short, value_parser, default_value = "false")]
        overwrite: bool,
    },
}
