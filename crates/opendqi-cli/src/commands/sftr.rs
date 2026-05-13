//! `opendqi sftr ...` subcommands — placeholders for milestone 0.4.

use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum SftrAction {
    /// Run a DQ scan against an SFTR file (planned).
    Scan {
        /// Path to the input file.
        input: PathBuf,
        /// Directory for report outputs.
        #[arg(long)]
        out: PathBuf,
    },
    /// Validate an SFTR file against its schema (planned).
    Validate {
        /// Path to the input file.
        input: PathBuf,
        /// XSD schema directory.
        #[arg(long)]
        xsd: Option<PathBuf>,
    },
    /// Normalize an SFTR file into Parquet (planned).
    Normalize {
        /// Path to the input file.
        input: PathBuf,
        /// Parquet output path.
        #[arg(long)]
        out: PathBuf,
    },
}

pub fn run(_action: SftrAction) -> Result<()> {
    println!("opendqi sftr: SFTR support is planned for a future milestone.");
    Ok(())
}
