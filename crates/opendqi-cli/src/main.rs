//! `opendqi` command-line interface.

#![forbid(unsafe_code)]

mod commands;

use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::{desktop, emir, feedback, sftr, smtp_test};

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
    /// Store-side workflow for TR feedback rows: list / resolve / stale.
    Feedback {
        #[command(subcommand)]
        action: feedback::FeedbackAction,
    },
    /// Start the local web UI (planned for milestone 0.2).
    Desktop(desktop::DesktopArgs),
    /// Validate an SMTP configuration YAML by sending a test email.
    /// Use `--dry-run` to check the config + env var without sending.
    /// See `docs/email-notifications.md`.
    SmtpTest(smtp_test::SmtpTestArgs),
}

fn main() -> Result<ExitCode> {
    init_tracing();
    let cli = Cli::parse();
    match cli.command {
        TopLevel::Emir { action } => emir::run(action),
        TopLevel::Sftr { action } => sftr::run(action),
        TopLevel::Feedback { action } => {
            feedback::run(action)?;
            Ok(ExitCode::SUCCESS)
        }
        TopLevel::Desktop(args) => {
            desktop::run(args)?;
            Ok(ExitCode::SUCCESS)
        }
        TopLevel::SmtpTest(args) => smtp_test::run(args),
    }
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}
