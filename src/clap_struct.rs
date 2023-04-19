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
        #[arg(long, value_parser, default_value = "false")]
        r#async: bool,
        /// transfer block size default 65536
        #[arg(long, short, value_parser, default_value = "65536")]
        block: usize,
        /// if service exists file, over write file
        #[arg(long, short, value_parser, default_value = "false")]
        overwrite: bool,
    },
    /// image path
    Image(ImageArgs),
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
        #[arg(long, value_parser, default_value = "false")]
        r#async: bool,
        /// transfer block size default 65536
        #[arg(long, short, value_parser, default_value = "65536")]
        block: usize,
        /// if service exists file, over write file
        #[arg(long, short, value_parser, default_value = "false")]
        overwrite: bool,
    },
}
