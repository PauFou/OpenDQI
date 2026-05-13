//! `opendqi` command-line interface.

#![forbid(unsafe_code)]

mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::{desktop, emir, sftr};

#[derive(Parser)]
#[command(
    name = "opendqi",
    version,
    about = "OpenDQI — local-first data quality engine for EMIR/SFTR reporting files.",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: TopLevel,
}

#[derive(Subcommand)]
enum TopLevel {
    /// EMIR-related commands.
    Emir {
        #[command(subcommand)]
        action: emir::EmirAction,
    },
    /// SFTR-related commands (planned).
    Sftr {
        #[command(subcommand)]
        action: sftr::SftrAction,
    },
    /// Start the local web UI (planned for milestone 0.2).
    Desktop(desktop::DesktopArgs),
}

fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    match cli.command {
        TopLevel::Emir { action } => emir::run(action),
        TopLevel::Sftr { action } => sftr::run(action),
        TopLevel::Desktop(args) => desktop::run(args),
    }
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}
