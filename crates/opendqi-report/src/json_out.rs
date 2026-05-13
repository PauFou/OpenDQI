//! `summary.json` writer.

use std::path::Path;

use anyhow::{Context, Result};
use opendqi_core::ScanSummary;

/// Write a pretty-printed `summary.json` for the given scan summary.
pub fn write_summary_json(path: &Path, summary: &ScanSummary) -> Result<()> {
    let file =
        std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    serde_json::to_writer_pretty(file, summary)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use opendqi_core::{DqDimension, Regime, ScanSummary, Severity};
    use std::collections::BTreeMap;

    fn fixture_summary() -> ScanSummary {
        let mut sev = BTreeMap::new();
        sev.insert(Severity::High, 3);
        sev.insert(Severity::Critical, 1);
        let mut dim = BTreeMap::new();
        dim.insert(DqDimension::Completeness, 3);
        dim.insert(DqDimension::Uniqueness, 1);
        ScanSummary {
            regime: Regime::Emir,
            files_processed: 1,
            records_processed: 10,
            issues_total: 4,
            issues_by_severity: sev,
            issues_by_dimension: dim,
            quality_score: 87.0,
            started_at: chrono::Utc.with_ymd_and_hms(2026, 5, 13, 12, 0, 0).unwrap(),
            finished_at: chrono::Utc.with_ymd_and_hms(2026, 5, 13, 12, 0, 1).unwrap(),
        }
    }

    #[test]
    fn writes_deterministic_json() {
        let path =
            std::env::temp_dir().join(format!("opendqi-summary-{}.json", std::process::id()));
        write_summary_json(&path, &fixture_summary()).unwrap();
        let a = std::fs::read(&path).unwrap();
        write_summary_json(&path, &fixture_summary()).unwrap();
        let b = std::fs::read(&path).unwrap();
        assert_eq!(a, b);
        let text = String::from_utf8(a).unwrap();
        assert!(text.contains("\"regime\": \"emir\""));
        assert!(text.contains("\"quality_score\""));
        std::fs::remove_file(&path).unwrap();
    }
}
