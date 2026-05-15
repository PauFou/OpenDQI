//! `report.html` writer using minijinja.

use std::path::Path;

use anyhow::{Context, Result};
use minijinja::{context, Environment};
use opendqi_core::{DqIssue, EvidenceItem, ScanSummary, Severity};
use serde::Serialize;

const TEMPLATE_SRC: &str = include_str!("templates/report.html.j2");

#[derive(Serialize)]
struct IssueView {
    check_id: String,
    severity: String,
    dimension: String,
    uti: String,
    record_id: String,
    field: String,
    message: String,
    source_file: String,
    evidence: Vec<EvidenceItem>,
}

impl From<&DqIssue> for IssueView {
    fn from(i: &DqIssue) -> Self {
        Self {
            check_id: i.check_id.clone(),
            severity: i.severity.to_string(),
            dimension: i.dimension.to_string(),
            uti: i.uti.clone().unwrap_or_default(),
            record_id: i.record_id.clone().unwrap_or_default(),
            field: i.field.clone().unwrap_or_default(),
            message: i.message.clone(),
            source_file: i.source_file.clone().unwrap_or_default(),
            evidence: i.evidence.clone(),
        }
    }
}

/// Write the HTML report.
pub fn write_report_html(
    path: &Path,
    summary: &ScanSummary,
    issues: &[DqIssue],
    sources: &[String],
) -> Result<()> {
    let mut env = Environment::new();
    env.add_template("report", TEMPLATE_SRC)
        .context("loading report template")?;
    let tmpl = env.get_template("report").context("getting template")?;

    // Top 20 issues, severity descending then check_id.
    let mut top: Vec<&DqIssue> = issues.iter().collect();
    top.sort_by(|a, b| {
        severity_rank(b.severity)
            .cmp(&severity_rank(a.severity))
            .then_with(|| a.check_id.cmp(&b.check_id))
    });
    let top_views: Vec<IssueView> = top.iter().take(20).map(|i| (*i).into()).collect();

    let by_severity: Vec<(String, u32)> = summary
        .issues_by_severity
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect();
    let by_dimension: Vec<(String, u32)> = summary
        .issues_by_dimension
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect();

    let rendered = tmpl
        .render(context! {
            summary => summary,
            top_issues => top_views,
            by_severity => by_severity,
            by_dimension => by_dimension,
            sources => sources,
            generated_at => summary.finished_at.to_rfc3339(),
        })
        .context("rendering report template")?;

    std::fs::write(path, rendered).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn severity_rank(s: Severity) -> u8 {
    match s {
        Severity::Critical => 4,
        Severity::High => 3,
        Severity::Warning => 2,
        Severity::Info => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use opendqi_core::{DqDimension, DqIssue, Regime, ScanSummary};
    use std::collections::BTreeMap;

    fn summary() -> ScanSummary {
        let now = Utc::now();
        ScanSummary {
            regime: Regime::Emir,
            files_processed: 1,
            records_processed: 1,
            issues_total: 1,
            issues_by_severity: BTreeMap::new(),
            issues_by_dimension: BTreeMap::new(),
            quality_score: 99.0,
            started_at: now,
            finished_at: now,
        }
    }

    #[test]
    fn report_html_renders_evidence_details_block() {
        let issue = DqIssue {
            check_id: "EMIR.CON.MODI_PRESERVES_UTI".into(),
            regime: Regime::Emir,
            severity: Severity::Warning,
            dimension: DqDimension::Consistency,
            record_id: Some("R1".into()),
            uti: Some("U1".into()),
            field: Some("prior_uti".into()),
            value: Some("U1".into()),
            message: "MODI preserves UTI".into(),
            source_file: Some("f.xml".into()),
            evidence: vec![EvidenceItem {
                field: "uti".into(),
                before: Some("U1".into()),
                after: Some("U1-NEW".into()),
                source_line: None,
            }],
        };
        let dir = std::env::temp_dir().join(format!("opendqi-ev-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("report.html");
        write_report_html(
            &path,
            &summary(),
            std::slice::from_ref(&issue),
            &["f.xml".into()],
        )
        .unwrap();
        let html = std::fs::read_to_string(&path).unwrap();
        assert!(
            html.contains("<details class=\"evidence\">"),
            "missing details block"
        );
        assert!(html.contains("U1-NEW"), "missing evidence `after` value");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn report_html_omits_details_when_no_evidence() {
        let issue = DqIssue {
            check_id: "EMIR.COMP.UTI_MISSING".into(),
            regime: Regime::Emir,
            severity: Severity::High,
            dimension: DqDimension::Completeness,
            record_id: None,
            uti: None,
            field: Some("uti".into()),
            value: None,
            message: "UTI missing".into(),
            source_file: None,
            evidence: Vec::new(),
        };
        let dir = std::env::temp_dir().join(format!("opendqi-noev-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("report.html");
        write_report_html(
            &path,
            &summary(),
            std::slice::from_ref(&issue),
            &["x".into()],
        )
        .unwrap();
        let html = std::fs::read_to_string(&path).unwrap();
        assert!(
            !html.contains("details class=\"evidence\""),
            "details should be absent"
        );
        std::fs::remove_file(&path).ok();
    }
}
