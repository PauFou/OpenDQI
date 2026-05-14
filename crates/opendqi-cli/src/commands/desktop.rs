//! `opendqi desktop` — launch the local web UI on http://127.0.0.1:<port>.

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct DesktopArgs {
    /// Port to bind the local web UI to.
    #[arg(long, default_value_t = 7878)]
    pub port: u16,
}

pub fn run(args: DesktopArgs) -> Result<()> {
    // Build a single-host tokio runtime in-thread; the server takes
    // over until Ctrl-C.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(opendqi_server::serve(args.port))
}
