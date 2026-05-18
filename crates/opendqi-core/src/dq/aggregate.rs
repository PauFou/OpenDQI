//! Online issue aggregator (Milestone 0.21).
//!
//! Reproduces a [`ScanSummary`] from a *stream* of issues without
//! retaining them — the summary seam of the streaming issue pipeline.
//! Folding [`IssueAggregator::observe`] over every issue yields a
//! `ScanSummary` **byte-identical** to the former hand-rolled
//! `build_summary` (same `BTreeMap` counts, same `issues_total`, and
//! `quality_score` via the single shared
//! [`crate::scoring::quality_score_from_counts`] arithmetic — no
//! formula drift).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use crate::model::{DqDimension, DqIssue, Regime, ScanSummary, Severity};
use crate::scoring::quality_score_from_counts;

/// Accumulates per-severity / per-dimension issue counts online.
#[derive(Debug, Default, Clone)]
pub struct IssueAggregator {
    by_severity: BTreeMap<Severity, u32>,
    by_dimension: BTreeMap<DqDimension, u32>,
    total: u32,
}

impl IssueAggregator {
    /// Create an empty aggregator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Tally one issue. Mirrors the old `build_summary` loop exactly:
    /// a severity/dimension entry exists only if observed at least
    /// once (no zero-filled keys).
    pub fn observe(&mut self, issue: &DqIssue) {
        *self.by_severity.entry(issue.severity).or_insert(0) += 1;
        *self.by_dimension.entry(issue.dimension).or_insert(0) += 1;
        self.total += 1;
    }

    /// Convenience: fold over an existing slice (Increment A feeds
    /// the aggregator from the still-materialised `Vec`; later
    /// increments call `observe` from the streaming sink instead).
    pub fn from_issues(issues: &[DqIssue]) -> Self {
        let mut agg = Self::new();
        for issue in issues {
            agg.observe(issue);
        }
        agg
    }

    /// Materialise the `ScanSummary`. `quality_score` is recomputed
    /// from the tallied severity counts via the exact shared
    /// formula, so the result is byte-identical to
    /// `quality_score(records, &issues)`.
    pub fn into_summary(
        self,
        regime: Regime,
        files_processed: u32,
        records_processed: u32,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
    ) -> ScanSummary {
        let count = |s: Severity| self.by_severity.get(&s).copied().unwrap_or(0);
        let quality_score = quality_score_from_counts(
            records_processed,
            count(Severity::Critical),
            count(Severity::High),
            count(Severity::Warning),
            count(Severity::Info),
        );
        ScanSummary {
            regime,
            files_processed,
            records_processed,
            issues_total: self.total,
            issues_by_severity: self.by_severity,
            issues_by_dimension: self.by_dimension,
            quality_score,
            started_at,
            finished_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(severity: Severity, dimension: DqDimension) -> DqIssue {
        DqIssue {
            check_id: "X".into(),
            regime: Regime::Emir,
            severity,
            dimension,
            record_id: None,
            uti: None,
            field: None,
            value: None,
            message: String::new(),
            source_file: None,
            evidence: Vec::new(),
        }
    }

    fn ts() -> DateTime<Utc> {
        DateTime::from_timestamp(0, 0).unwrap()
    }

    #[test]
    fn empty_is_perfect_score_and_empty_maps() {
        let s = IssueAggregator::from_issues(&[]).into_summary(Regime::Emir, 1, 10, ts(), ts());
        assert_eq!(s.issues_total, 0);
        assert!(s.issues_by_severity.is_empty());
        assert!(s.issues_by_dimension.is_empty());
        assert_eq!(s.quality_score, 100.0);
        assert_eq!(s.files_processed, 1);
        assert_eq!(s.records_processed, 10);
    }

    #[test]
    fn counts_and_score_match_the_exact_formula() {
        let issues = vec![
            issue(Severity::Critical, DqDimension::Completeness),
            issue(Severity::Critical, DqDimension::Validity),
            issue(Severity::High, DqDimension::Accuracy),
        ];
        let s = IssueAggregator::from_issues(&issues).into_summary(Regime::Sftr, 2, 10, ts(), ts());
        assert_eq!(s.issues_total, 3);
        assert_eq!(s.issues_by_severity.get(&Severity::Critical), Some(&2));
        assert_eq!(s.issues_by_severity.get(&Severity::High), Some(&1));
        assert_eq!(s.issues_by_severity.get(&Severity::Warning), None);
        assert_eq!(
            s.issues_by_dimension.get(&DqDimension::Completeness),
            Some(&1)
        );
        assert_eq!(s.issues_by_dimension.get(&DqDimension::Validity), Some(&1));
        assert_eq!(s.issues_by_dimension.get(&DqDimension::Accuracy), Some(&1));
        // 100 - (25*2/10 + 10*1/10) = 100 - (5.0 + 1.0) = 94.0
        assert_eq!(s.quality_score, 94.0);
        assert_eq!(s.regime, Regime::Sftr);
    }

    #[test]
    fn zero_records_is_always_perfect_score() {
        let issues = vec![issue(Severity::Critical, DqDimension::Completeness)];
        let s = IssueAggregator::from_issues(&issues).into_summary(Regime::Emir, 0, 0, ts(), ts());
        assert_eq!(s.quality_score, 100.0);
        assert_eq!(s.issues_total, 1);
    }

    #[test]
    fn observe_stream_equals_from_slice() {
        let issues = vec![
            issue(Severity::Warning, DqDimension::Timeliness),
            issue(Severity::Info, DqDimension::Consistency),
        ];
        let mut streamed = IssueAggregator::new();
        for i in &issues {
            streamed.observe(i);
        }
        let a = streamed.into_summary(Regime::Emir, 1, 5, ts(), ts());
        let b = IssueAggregator::from_issues(&issues).into_summary(Regime::Emir, 1, 5, ts(), ts());
        assert_eq!(a.issues_by_severity, b.issues_by_severity);
        assert_eq!(a.issues_by_dimension, b.issues_by_dimension);
        assert_eq!(a.issues_total, b.issues_total);
        assert_eq!(a.quality_score, b.quality_score);
    }
}
