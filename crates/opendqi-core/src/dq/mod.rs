//! Data-quality check trait and the MVP EMIR check registry.

use chrono::{DateTime, NaiveDate, Utc};

use crate::config::Thresholds;
use crate::model::{DqDimension, DqIssue, EmirRecord, Severity};

mod abnormal_maturity;
mod duplicate_uti;
mod late_reporting;
mod missing_uti;
mod missing_valuation;

pub use abnormal_maturity::AbnormalMaturity;
pub use duplicate_uti::DuplicateUti;
pub use late_reporting::LateReporting;
pub use missing_uti::MissingUti;
pub use missing_valuation::MissingValuation;

/// Read-only context passed to every check.
///
/// Injecting `today` / `now` keeps checks deterministic: tests can pin
/// a specific date and observe identical issue output across runs.
#[derive(Debug, Clone)]
pub struct CheckContext {
    /// Threshold configuration.
    pub thresholds: Thresholds,
    /// Reference calendar date (UTC).
    pub today: NaiveDate,
    /// Reference instant.
    pub now: DateTime<Utc>,
}

impl CheckContext {
    /// Build a context using the system clock and default thresholds.
    pub fn now_with_defaults() -> Self {
        let now = Utc::now();
        Self {
            thresholds: Thresholds::default(),
            today: now.date_naive(),
            now,
        }
    }
}

/// A data-quality check. Implementations are pure functions of the
/// input records and the context.
pub trait Check: Send + Sync {
    /// Stable identifier, e.g. `EMIR.COMP.UTI_MISSING`.
    fn id(&self) -> &'static str;
    /// The DQ dimension this check belongs to.
    fn dimension(&self) -> DqDimension;
    /// Default severity for issues raised by this check.
    fn severity(&self) -> Severity;
    /// Execute the check against the given records.
    fn run(&self, records: &[EmirRecord], ctx: &CheckContext) -> Vec<DqIssue>;
}

/// The five MVP EMIR checks, returned in a stable order.
pub fn default_checks() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(MissingUti),
        Box::new(MissingValuation),
        Box::new(AbnormalMaturity),
        Box::new(DuplicateUti),
        Box::new(LateReporting),
    ]
}

/// Run every check in `checks` against `records` and return the
/// concatenated issues, sorted deterministically.
pub fn run_all(
    checks: &[Box<dyn Check>],
    records: &[EmirRecord],
    ctx: &CheckContext,
) -> Vec<DqIssue> {
    let mut issues: Vec<DqIssue> = checks.iter().flat_map(|c| c.run(records, ctx)).collect();
    issues.sort_by(|a, b| {
        a.check_id
            .cmp(&b.check_id)
            .then_with(|| a.source_file.cmp(&b.source_file))
            .then_with(|| a.record_id.cmp(&b.record_id))
    });
    issues
}
