//! `report.html` writer using minijinja.

use std::path::Path;

use anyhow::{Context, Result};
use minijinja::{context, Environment};
use opendqi_core::{DqIssue, DqiIndicator, EvidenceItem, ScanSummary, Severity};
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

/// Indicator view for the optional "Data Quality Pack" HTML
/// section (v0.15 +). `rate_pct` is a pre-formatted percentage
/// string so the template stays free of float arithmetic.
#[derive(Serialize)]
struct IndicatorView {
    indicator_id: String,
    dimension: String,
    table_scope: String,
    numerator: u64,
    denominator: u64,
    rate_pct: String,
    threshold_amber_pct: String,
    threshold_red_pct: String,
    status: String,
    description: String,
}

impl From<&DqiIndicator> for IndicatorView {
    fn from(i: &DqiIndicator) -> Self {
        let pct = |v: Option<f64>| match v {
            Some(r) => format!("{:.2}%", r * 100.0),
            None => String::new(),
        };
        Self {
            indicator_id: i.indicator_id.clone(),
            dimension: i.dimension.to_string(),
            table_scope: i.table_scope.clone(),
            numerator: i.numerator,
            denominator: i.denominator,
            rate_pct: pct(i.rate),
            threshold_amber_pct: pct(i.threshold_amber),
            threshold_red_pct: pct(i.threshold_red),
            status: i.status.to_string(),
            description: i.description.clone(),
        }
    }
}

/// Write the HTML report.
///
/// Backward-compatible wrapper around
/// [`write_report_html_with_dqi`] that passes `None` for the
/// Data Quality Pack indicators (the "Data Quality Pack"
/// section is then invisible — byte-identical output for all
/// callers that haven't opted into the pack).
pub fn write_report_html(
    path: &Path,
    summary: &ScanSummary,
    issues: &[DqIssue],
    sources: &[String],
) -> Result<()> {
    write_report_html_with_dqi(path, summary, issues, sources, None)
}

/// Write the HTML report with the optional "Data Quality Pack"
/// section (v0.15).
///
/// When `indicators` is `None` or empty, the section is
/// omitted entirely so existing reports (`emir scan`, `tr-audit`,
/// `collateral-audit`, …) render byte-identically to the
/// pre-v0.15 template output.
pub fn write_report_html_with_dqi(
    path: &Path,
    summary: &ScanSummary,
    issues: &[DqIssue],
    sources: &[String],
    indicators: Option<&[DqiIndicator]>,
) -> Result<()> {
    let mut env = Environment::new();
    env.add_template("report", TEMPLATE_SRC)
        .context("loading report template")?;
    let tmpl = env.get_template("report").context("getting template")?;

    // Top 20 issues, severity descending then check_id — via the
    // bounded `TopIssues` selector (same logic the streaming
    // `run_scan` pipeline uses, so both pick the same set).
    let mut top = TopIssues::with_capacity(20);
    for i in issues {
        top.offer(i);
    }
    let top_owned = top.into_sorted();
    let top_views: Vec<IssueView> = top_owned.iter().map(|i| i.into()).collect();

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

    let indicator_views: Vec<IndicatorView> = indicators
        .unwrap_or(&[])
        .iter()
        .map(IndicatorView::from)
        .collect();

    let rendered = tmpl
        .render(context! {
            summary => summary,
            top_issues => top_views,
            by_severity => by_severity,
            by_dimension => by_dimension,
            sources => sources,
            indicators => indicator_views,
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

/// `Greater` when `a` is **lower** display priority than `b` (i.e.
/// `a` is the eviction candidate): severity rank descending, then
/// `check_id` ascending. Single definition of the report's display
/// order, shared by [`TopIssues`] and its `into_sorted`.
fn lower_priority(a: &DqIssue, b: &DqIssue) -> std::cmp::Ordering {
    severity_rank(b.severity)
        .cmp(&severity_rank(a.severity))
        .then_with(|| a.check_id.cmp(&b.check_id))
}

/// Heap wrapper whose `Ord` makes the **lowest** display-priority
/// issue the `BinaryHeap` maximum (so it is the one evicted when the
/// selector is over capacity).
struct Ranked(DqIssue);
impl PartialEq for Ranked {
    fn eq(&self, o: &Self) -> bool {
        lower_priority(&self.0, &o.0) == std::cmp::Ordering::Equal
    }
}
impl Eq for Ranked {}
impl Ord for Ranked {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        lower_priority(&self.0, &o.0)
    }
}
impl PartialOrd for Ranked {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

/// Streaming, bounded top-N issue selector by the report's display
/// order (severity rank descending, then `check_id` ascending).
/// O(`cap`) memory — runs over a streamed scan without retaining
/// every issue. Clones only the issues actually kept.
pub struct TopIssues {
    cap: usize,
    heap: std::collections::BinaryHeap<Ranked>,
}

impl TopIssues {
    /// Selector retaining at most `cap` highest-priority issues.
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            cap,
            heap: std::collections::BinaryHeap::new(),
        }
    }

    /// Offer one issue; kept only if among the current top `cap`.
    pub fn offer(&mut self, issue: &DqIssue) {
        if self.cap == 0 {
            return;
        }
        if self.heap.len() < self.cap {
            self.heap.push(Ranked(issue.clone()));
            return;
        }
        // `heap.peek()` is the lowest-priority kept issue; replace it
        // only if the new issue is strictly higher priority.
        if let Some(worst) = self.heap.peek() {
            if lower_priority(issue, &worst.0) == std::cmp::Ordering::Less {
                self.heap.pop();
                self.heap.push(Ranked(issue.clone()));
            }
        }
    }

    /// The retained issues in report display order (highest priority
    /// first) — identical ordering to the former
    /// `sort_by(sev desc, check_id).take(cap)`.
    pub fn into_sorted(self) -> Vec<DqIssue> {
        let mut v: Vec<DqIssue> = self.heap.into_iter().map(|r| r.0).collect();
        v.sort_by(lower_priority);
        v
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
    fn dqi_section_omitted_when_indicators_none() {
        // The default write_report_html signature passes None → the
        // "Data Quality Pack" <h2> must NOT appear (existing
        // reports stay visually identical).
        let dir = std::env::temp_dir().join(format!("opendqi-dqinone-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("report.html");
        write_report_html(&path, &summary(), &[], &["x".into()]).unwrap();
        let html = std::fs::read_to_string(&path).unwrap();
        assert!(
            !html.contains("Data Quality Pack"),
            "DQI section should be absent for non-DQI callers"
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn dqi_section_rendered_when_indicators_supplied() {
        use opendqi_core::{DqDimension, DqiIndicator, DqiStatus, Regime};
        let ind = DqiIndicator {
            indicator_id: "DQI_VAL_MISSING".into(),
            regime: Regime::Emir,
            dimension: DqDimension::Completeness,
            table_scope: "TSR".into(),
            numerator: 5,
            denominator: 100,
            rate: Some(0.05),
            threshold_amber: Some(0.01),
            threshold_red: Some(0.05),
            status: DqiStatus::Amber,
            description: "test indicator".into(),
        };
        let dir = std::env::temp_dir().join(format!("opendqi-dqiyes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("report.html");
        write_report_html_with_dqi(
            &path,
            &summary(),
            &[],
            &["x".into()],
            Some(std::slice::from_ref(&ind)),
        )
        .unwrap();
        let html = std::fs::read_to_string(&path).unwrap();
        assert!(html.contains("Data Quality Pack"), "DQI section missing");
        assert!(
            html.contains("DQI_VAL_MISSING"),
            "indicator_id not rendered"
        );
        assert!(html.contains("5.00%"), "rate not rendered as percent");
        assert!(
            html.contains(r#"<span class="dqi-status amber">amber</span>"#),
            "amber status pill not rendered"
        );
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
