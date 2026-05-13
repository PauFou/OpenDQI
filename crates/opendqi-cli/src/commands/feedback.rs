//! `opendqi feedback ...` — store-side workflow for TR feedback rows.
//!
//! These commands operate on the `feedbacks` table written by the
//! `opendqi {emir,sftr} feedback` ingestion subcommands. They do not
//! parse any XML; they only read and write the local SQLite store.

use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::Subcommand;
use opendqi_core::Regime;
use tracing::info;

#[derive(Subcommand)]
pub enum FeedbackAction {
    /// List feedback rows in the store, optionally filtered by regime,
    /// UTI, or status.
    List {
        /// Path to the SQLite history store.
        #[arg(long)]
        store: PathBuf,
        /// Filter on regime: `emir` or `sftr`.
        #[arg(long)]
        regime: Option<String>,
        /// Filter on UTI.
        #[arg(long)]
        uti: Option<String>,
        /// Filter on status: `open`, `resolved`, or `stale`.
        #[arg(long)]
        status: Option<String>,
    },
    /// Mark every feedback row for `uti` as `resolved`.
    Resolve {
        /// Path to the SQLite history store.
        #[arg(long)]
        store: PathBuf,
        /// UTI whose feedback rows should be resolved.
        #[arg(long)]
        uti: String,
    },
    /// Mark every feedback row for `uti` as `stale` (no longer
    /// relevant; do not surface again).
    Stale {
        /// Path to the SQLite history store.
        #[arg(long)]
        store: PathBuf,
        /// UTI whose feedback rows should be marked stale.
        #[arg(long)]
        uti: String,
    },
}

pub fn run(action: FeedbackAction) -> Result<()> {
    match action {
        FeedbackAction::List {
            store,
            regime,
            uti,
            status,
        } => run_list(&store, regime.as_deref(), uti.as_deref(), status.as_deref()),
        FeedbackAction::Resolve { store, uti } => run_update(&store, &uti, "resolved"),
        FeedbackAction::Stale { store, uti } => run_update(&store, &uti, "stale"),
    }
}

fn parse_regime(s: &str) -> Result<Regime> {
    match s.to_ascii_lowercase().as_str() {
        "emir" => Ok(Regime::Emir),
        "sftr" => Ok(Regime::Sftr),
        other => Err(anyhow::anyhow!(
            "unknown regime '{other}' — expected 'emir' or 'sftr'"
        )),
    }
}

fn validate_status(s: &str) -> Result<&str> {
    match s {
        "open" | "resolved" | "stale" => Ok(s),
        other => Err(anyhow::anyhow!(
            "unknown status '{other}' — expected 'open', 'resolved', or 'stale'"
        )),
    }
}

fn run_list(
    store_path: &std::path::Path,
    regime: Option<&str>,
    uti: Option<&str>,
    status: Option<&str>,
) -> Result<()> {
    let store = opendqi_store::open_store(store_path)
        .with_context(|| format!("opening history store at {}", store_path.display()))?;
    let regime = match regime {
        Some(s) => Some(parse_regime(s)?),
        None => None,
    };
    let status = match status {
        Some(s) => Some(validate_status(s)?),
        None => None,
    };
    let rows = store
        .list_feedbacks(regime, uti, status)
        .context("listing feedbacks")?;

    if rows.is_empty() {
        println!("No matching feedback rows.");
        return Ok(());
    }
    println!(
        "{:<6}  {:<5}  {:<10}  {:<8}  {:<30}  REASON",
        "ID", "REG", "TYPE", "STATUS", "UTI"
    );
    for r in &rows {
        let regime_str = match r.regime {
            Regime::Emir => "EMIR",
            Regime::Sftr => "SFTR",
        };
        let reason = match (&r.reason_code, &r.reason_description) {
            (Some(c), Some(d)) => format!("[{c}] {d}"),
            (None, Some(d)) => d.clone(),
            (Some(c), None) => format!("[{c}]"),
            (None, None) => String::new(),
        };
        let ts = DateTime::<Utc>::from_timestamp(r.status_set_at, 0)
            .map(|d| d.to_rfc3339())
            .unwrap_or_default();
        println!(
            "{:<6}  {:<5}  {:<10}  {:<8}  {:<30}  {} (status_set={ts})",
            r.feedback_id,
            regime_str,
            r.feedback_type,
            r.status,
            r.uti.as_deref().unwrap_or("(no UTI)"),
            reason,
        );
    }
    println!("\n{} row(s).", rows.len());
    Ok(())
}

fn run_update(store_path: &std::path::Path, uti: &str, new_status: &str) -> Result<()> {
    let store = opendqi_store::open_store(store_path)
        .with_context(|| format!("opening history store at {}", store_path.display()))?;
    let updated = store
        .update_feedback_status(uti, new_status)
        .with_context(|| format!("updating feedback status for UTI {uti}"))?;
    info!(uti, new_status, updated, "feedback status updated");
    println!("Updated {updated} feedback row(s) for UTI {uti} → {new_status}.");
    Ok(())
}
