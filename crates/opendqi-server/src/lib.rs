//! OpenDQI local web UI — axum-based.
//!
//! Single-host, single-user tool that wraps `opendqi {emir,sftr} scan`
//! behind an HTTP form on `http://127.0.0.1:7878`. Users drop an XML
//! or Parquet file, pick a regime + mode, and download the resulting
//! `report.html` / `summary.json` / `issues.csv`.
//!
//! Designed to run as a local desktop tool: no auth, binds 127.0.0.1
//! only, and stores scan artifacts in a per-process temp directory.

#![forbid(unsafe_code)]

mod handlers;
mod scan;
mod state;
mod templates;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::routing::{get, post};
use axum::Router;

pub use state::AppState;
use tower_http::trace::TraceLayer;

/// Build the axum router. Exposed for tests.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(handlers::index))
        .route("/api/scan", post(handlers::scan))
        .route("/scans/:id", get(handlers::scan_results))
        .route("/scans/:id/:file", get(handlers::scan_download))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

/// Bind to `127.0.0.1:port` and serve the UI until Ctrl-C.
pub async fn serve(port: u16) -> Result<()> {
    let state = Arc::new(AppState::new()?);
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding to {addr}"))?;
    tracing::info!(%addr, "opendqi-server listening");
    println!("opendqi desktop — open http://{addr} in your browser (Ctrl-C to stop).");
    axum::serve(listener, router(state).into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serving HTTP")?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("Ctrl-C received, shutting down");
}
