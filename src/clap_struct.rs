use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
pub enum Opt {
    /// push file
    Push {
        /// save dir
        #[arg(long, short, value_parser)]
        dir: Option<PathBuf>,
        /// async write
        #[arg(long, value_parser, default_value = "false")]
        r#async: bool,
        /// transfer block size default 65536
        #[arg(long, short, value_parser, default_value = "65536")]
        block: usize,
        /// local file
        #[arg(value_parser)]
        file: PathBuf,
    },
    /// create config
    Create,
}
