//! `opendqi feedback ...` — store-side workflow for TR feedback rows.
//!
//! These commands operate on the `feedbacks` table written by the
//! `opendqi {emir,sftr} feedback` ingestion subcommands. They do not
//! parse any XML; they only read and write the local SQLite store.

use std::collections::BTreeMap;
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
    /// Generate a rejection-analytics report over every feedback
    /// row in the store: top rejection causes, repeated rejected
    /// UTIs, age buckets, rejected-then-accepted detection,
    /// `rejection_profile.yml` export. Outputs
    /// `rejection_summary.json`, `rejections.csv`,
    /// `rejection_profile.yml`, `rejection_report.html`.
    Analytics {
        /// Path to the SQLite history store.
        #[arg(long)]
        store: PathBuf,
        /// Filter analytics by regime (`emir` or `sftr`). Default:
        /// all regimes mixed.
        #[arg(long)]
        regime: Option<String>,
        /// Directory where reports are written.
        #[arg(long)]
        out: PathBuf,
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
        FeedbackAction::Analytics { store, regime, out } => {
            run_analytics(&store, regime.as_deref(), &out)
        }
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

/// Threshold above which a UTI is flagged as "repeated rejection".
const REPEATED_REJECTION_THRESHOLD: i64 = 3;
/// Days of inactivity above which an `open` feedback row is flagged
/// as stale.
const STALE_REJECTION_AGE_DAYS: i64 = 7;

fn run_analytics(
    store_path: &std::path::Path,
    regime_filter: Option<&str>,
    out: &std::path::Path,
) -> Result<()> {
    let regime = match regime_filter {
        Some(s) => Some(parse_regime(s)?),
        None => None,
    };
    let store = opendqi_store::open_store(store_path)
        .with_context(|| format!("opening history store at {}", store_path.display()))?;

    let all = store
        .list_feedbacks(regime, None, None)
        .context("listing feedbacks")?;
    let top_causes = store
        .count_feedbacks_by_reason(regime)
        .context("aggregating reason codes")?;
    let by_uti = store
        .count_feedbacks_by_uti(regime)
        .context("aggregating UTIs")?;

    let now_ts = Utc::now().timestamp();
    let stale_cutoff = now_ts - (STALE_REJECTION_AGE_DAYS * 86_400);

    // Age buckets (days since ingestion).
    let mut age_buckets: BTreeMap<&'static str, u32> =
        BTreeMap::from([("0-1d", 0), ("1-7d", 0), ("7-30d", 0), ("30d+", 0)]);
    for row in &all {
        let age = (now_ts - row.ingested_at).max(0);
        let bucket = if age <= 86_400 {
            "0-1d"
        } else if age <= 7 * 86_400 {
            "1-7d"
        } else if age <= 30 * 86_400 {
            "7-30d"
        } else {
            "30d+"
        };
        *age_buckets.get_mut(bucket).unwrap() += 1;
    }

    // Repeated rejections: UTIs with count >= threshold.
    let repeated: Vec<(String, i64)> = by_uti
        .iter()
        .filter(|(u, n)| u != "(none)" && *n >= REPEATED_REJECTION_THRESHOLD)
        .cloned()
        .collect();

    // Stale rejections: rows currently `open` with status_set_at older than cutoff.
    let stale: Vec<&opendqi_store::FeedbackRow> = all
        .iter()
        .filter(|r| r.status == "open" && r.status_set_at < stale_cutoff)
        .collect();

    // Rejected-then-accepted: for each rejected UTI, check whether a
    // later NEWT exists in the EMIR records table.
    let mut rejected_then_accepted: Vec<(String, i64, i64)> = Vec::new();
    for row in &all {
        if !matches!(row.feedback_type.as_str(), "rejected" | "missing") {
            continue;
        }
        let uti = match row.uti.as_deref() {
            Some(u) if !u.trim().is_empty() => u,
            _ => continue,
        };
        let priors = store
            .load_prior_emir(&[uti], i64::MAX)
            .context("loading prior EMIR records for rejected UTI")?;
        if let Some(p) = priors.iter().find(|p| {
            p.action_type
                .as_deref()
                .map(|a| a.eq_ignore_ascii_case("NEWT"))
                .unwrap_or(false)
                && p.reporting_timestamp
                    .map(|t| t.timestamp() > row.ingested_at)
                    .unwrap_or(false)
        }) {
            let accepted_at = p.reporting_timestamp.unwrap().timestamp();
            rejected_then_accepted.push((uti.to_owned(), row.ingested_at, accepted_at));
        }
    }

    std::fs::create_dir_all(out)
        .with_context(|| format!("creating output directory {}", out.display()))?;

    // rejection_summary.json
    let summary = serde_json::json!({
        "total_feedbacks": all.len(),
        "regime_filter": regime_filter,
        "top_causes": top_causes.iter().take(10).map(|(c, n)| serde_json::json!({"reason_code": c, "count": n})).collect::<Vec<_>>(),
        "repeated_rejections": repeated.iter().map(|(u, n)| serde_json::json!({"uti": u, "count": n})).collect::<Vec<_>>(),
        "age_buckets": age_buckets,
        "stale_open_count": stale.len(),
        "rejected_then_accepted": rejected_then_accepted.iter().map(|(u, r, a)| serde_json::json!({"uti": u, "rejected_at": r, "accepted_at": a})).collect::<Vec<_>>(),
    });
    std::fs::write(
        out.join("rejection_summary.json"),
        serde_json::to_string_pretty(&summary)?,
    )?;

    // rejections.csv — flat dump filtered to open / stale.
    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_path(out.join("rejections.csv"))
        .with_context(|| format!("creating rejections.csv at {}", out.display()))?;
    writer.write_record([
        "feedback_id",
        "regime",
        "uti",
        "feedback_type",
        "reason_code",
        "reason_description",
        "reported_field",
        "status",
        "ingested_at",
    ])?;
    for r in all
        .iter()
        .filter(|r| r.status == "open" || r.status == "stale")
    {
        let regime_str = match r.regime {
            Regime::Emir => "EMIR",
            Regime::Sftr => "SFTR",
        };
        writer.write_record([
            r.feedback_id.to_string().as_str(),
            regime_str,
            r.uti.as_deref().unwrap_or(""),
            r.feedback_type.as_str(),
            r.reason_code.as_deref().unwrap_or(""),
            r.reason_description.as_deref().unwrap_or(""),
            r.reported_field.as_deref().unwrap_or(""),
            r.status.as_str(),
            DateTime::<Utc>::from_timestamp(r.ingested_at, 0)
                .map(|d| d.to_rfc3339())
                .unwrap_or_default()
                .as_str(),
        ])?;
    }
    writer.flush()?;

    // rejection_profile.yml — human-readable patterns.
    let mut yml = String::from("# OpenDQI rejection profile — auto-generated.\n# Copy the patterns below into your pre-submission check pack.\n\nprofile:\n");
    yml.push_str(&format!(
        "  generated_at: \"{}\"\n",
        Utc::now().to_rfc3339()
    ));
    yml.push_str(&format!("  total_feedbacks: {}\n", all.len()));
    yml.push_str("  top_causes:\n");
    for (code, n) in top_causes.iter().take(10) {
        let suggested = suggested_check_for_reason(code);
        yml.push_str(&format!(
            "    - reason_code: \"{code}\"\n      count: {n}\n      suggested_check: \"{suggested}\"\n"
        ));
    }
    if !repeated.is_empty() {
        yml.push_str("  repeated_rejected_utis:\n");
        for (uti, n) in &repeated {
            yml.push_str(&format!("    - uti: \"{uti}\"\n      count: {n}\n"));
        }
    }
    std::fs::write(out.join("rejection_profile.yml"), yml)?;

    // rejection_report.html — minimal HTML.
    let html = format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>OpenDQI rejection analytics</title>\
<style>body{{font-family:system-ui,sans-serif;max-width:980px;margin:2rem auto;color:#222}}h1,h2{{margin-top:2rem}}table{{border-collapse:collapse;width:100%;margin-top:.6rem}}th,td{{padding:.4rem .6rem;border-bottom:1px solid #ddd;text-align:left}}.k{{color:#666;font-size:.9rem}}</style></head><body>\
<h1>Rejection analytics</h1>\
<p class=\"k\">Total feedback rows: <b>{total}</b>. Regime filter: <b>{regime}</b>.</p>\
<h2>Top rejection causes</h2><table><tr><th>Reason code</th><th>Count</th></tr>{causes}</table>\
<h2>Repeated rejected UTIs (≥ {rep_th})</h2><table><tr><th>UTI</th><th>Count</th></tr>{repeated}</table>\
<h2>Age buckets (since ingestion)</h2><table><tr><th>Bucket</th><th>Count</th></tr>{ages}</table>\
<h2>Rejected then accepted</h2><table><tr><th>UTI</th><th>Rejected at (Unix)</th><th>Accepted at (Unix)</th></tr>{rta}</table>\
<p class=\"k\">Generated by OpenDQI at {now}.</p></body></html>",
        total = all.len(),
        regime = regime_filter.unwrap_or("(all)"),
        rep_th = REPEATED_REJECTION_THRESHOLD,
        causes = top_causes
            .iter()
            .take(10)
            .map(|(c, n)| format!("<tr><td>{c}</td><td>{n}</td></tr>"))
            .collect::<String>(),
        repeated = repeated
            .iter()
            .map(|(u, n)| format!("<tr><td>{u}</td><td>{n}</td></tr>"))
            .collect::<String>(),
        ages = age_buckets
            .iter()
            .map(|(b, n)| format!("<tr><td>{b}</td><td>{n}</td></tr>"))
            .collect::<String>(),
        rta = rejected_then_accepted
            .iter()
            .map(|(u, r, a)| format!("<tr><td>{u}</td><td>{r}</td><td>{a}</td></tr>"))
            .collect::<String>(),
        now = Utc::now().to_rfc3339(),
    );
    std::fs::write(out.join("rejection_report.html"), html)?;

    println!(
        "Analysed {} feedback row(s). Repeated rejections: {}. Stale open: {}. Rejected→accepted: {}.",
        all.len(),
        repeated.len(),
        stale.len(),
        rejected_then_accepted.len(),
    );
    println!("Report: {}", out.join("rejection_report.html").display());
    Ok(())
}

/// Best-effort mapping from TR-side `reason_code` to a canonical
/// OpenDQI check ID. Used by `rejection_profile.yml` so the
/// `EMIR.PSC.LIKELY_REJECTION_PATTERN` pre-submission check can flag
/// matching records on the next scan.
///
/// Unknown codes fall back to a free-form sentinel so the profile
/// stays self-describing — only the canonical IDs are dispatched.
fn suggested_check_for_reason(code: &str) -> String {
    let upper = code.trim().to_ascii_uppercase();
    match upper.as_str() {
        // Completeness-style ESMA codes (heuristic mapping).
        "VAL01" | "MISSING_NOTIONAL_CCY" => "EMIR.COMP.NOTIONAL_CURRENCY_MISSING".into(),
        "VAL02" | "MISSING_NOTIONAL" | "ZERO_NOTIONAL" => "EMIR.ACC.ZERO_NOTIONAL".into(),
        "VAL03" | "NEGATIVE_NOTIONAL" => "EMIR.ACC.NEGATIVE_NOTIONAL".into(),
        "VAL05" | "MISSING_VALUATION" => "EMIR.COMP.VALUATION_MISSING".into(),
        "VAL10" | "MISSING_CP1_LEI" => "EMIR.COMP.COUNTERPARTY_1_MISSING".into(),
        "VAL11" | "MISSING_CP2_LEI" => "EMIR.COMP.COUNTERPARTY_2_MISSING".into(),
        "VAL12" | "MISSING_UTI" => "EMIR.COMP.UTI_MISSING".into(),
        _ => format!("(no canonical check for {code})"),
    }
}
