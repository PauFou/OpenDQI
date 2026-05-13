//! `report.html` writer using minijinja.

use std::path::Path;

use anyhow::{Context, Result};
use minijinja::{context, Environment};
use opendqi_core::{DqIssue, ScanSummary, Severity};
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
