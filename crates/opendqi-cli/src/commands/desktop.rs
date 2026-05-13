//! `opendqi desktop` — local web UI placeholder for milestone 0.2.

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct DesktopArgs {
    /// Port to bind the local web UI to.
    #[arg(long, default_value_t = 7878)]
    pub port: u16,
}

pub fn run(args: DesktopArgs) -> Result<()> {
    println!(
        "opendqi desktop: local web UI is planned for milestone 0.2 (would bind to http://localhost:{}).",
        args.port
    );
    Ok(())
}
