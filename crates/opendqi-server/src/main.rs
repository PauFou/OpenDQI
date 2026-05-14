//! Binary entry point for the OpenDQI local web UI. Reads the
//! `OPENDQI_DESKTOP_PORT` env var if set, otherwise binds 7878.

#![forbid(unsafe_code)]

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .try_init();
    let port = std::env::var("OPENDQI_DESKTOP_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(7878);
    opendqi_server::serve(port).await
}
