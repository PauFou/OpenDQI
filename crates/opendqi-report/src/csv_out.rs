//! `issues.csv` writer.

use std::path::Path;

use anyhow::{Context, Result};
use opendqi_core::DqIssue;

/// Write the issues to a CSV with a stable column order.
///
/// Rows are pre-sorted by `(check_id, source_file, record_id)` so the
/// output is reproducible across runs.
pub fn write_issues_csv(path: &Path, issues: &[DqIssue]) -> Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_path(path)
        .with_context(|| format!("creating {}", path.display()))?;

    writer.write_record([
        "check_id",
        "regime",
        "severity",
        "dimension",
        "record_id",
        "uti",
        "field",
        "value",
        "message",
        "source_file",
        "evidence_json",
    ])?;

    let mut sorted: Vec<&DqIssue> = issues.iter().collect();
    sorted.sort_by(|a, b| {
        a.check_id
            .cmp(&b.check_id)
            .then_with(|| a.source_file.cmp(&b.source_file))
            .then_with(|| a.record_id.cmp(&b.record_id))
            .then_with(|| a.uti.cmp(&b.uti))
    });

    for issue in sorted {
        let evidence_json = if issue.evidence.is_empty() {
            String::new()
        } else {
            serde_json::to_string(&issue.evidence).unwrap_or_default()
        };
        writer.write_record([
            issue.check_id.as_str(),
            &issue.regime.to_string(),
            &issue.severity.to_string(),
            &issue.dimension.to_string(),
            issue.record_id.as_deref().unwrap_or(""),
            issue.uti.as_deref().unwrap_or(""),
            issue.field.as_deref().unwrap_or(""),
            issue.value.as_deref().unwrap_or(""),
            issue.message.as_str(),
            issue.source_file.as_deref().unwrap_or(""),
            evidence_json.as_str(),
        ])?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendqi_core::{DqDimension, Regime, Severity};

    fn iss(check: &str, uti: &str) -> DqIssue {
        DqIssue {
            check_id: check.into(),
            regime: Regime::Emir,
            severity: Severity::High,
            dimension: DqDimension::Completeness,
            record_id: Some("1".into()),
            uti: Some(uti.into()),
            field: None,
            value: None,
            message: "x".into(),
            source_file: Some("f.csv".into()),
            evidence: Vec::new(),
        }
    }

    #[test]
    fn writes_csv_with_headers() {
        let path = std::env::temp_dir().join(format!("opendqi-issues-{}.csv", std::process::id()));
        write_issues_csv(&path, &[iss("B", "u1"), iss("A", "u2")]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("check_id,regime,severity"));
        // sorted: A before B
        let a_pos = text.find(",A,").or_else(|| text.find("\nA,"));
        let b_pos = text.find(",B,").or_else(|| text.find("\nB,"));
        if let (Some(a), Some(b)) = (a_pos, b_pos) {
            assert!(a < b);
        }
        std::fs::remove_file(&path).unwrap();
    }
}
