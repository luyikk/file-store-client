use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
pub enum Opt {
    /// push file
    Push {
        /// save dir
        #[arg(long, short, value_parser)]
        dir: Option<PathBuf>,
        /// local file
        #[arg(value_parser)]
        file: PathBuf,
    },
    /// create config
    Create,
}
