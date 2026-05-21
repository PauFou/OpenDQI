//! `DQI_ANOMALY_RATE` — share of TSR records that exhibit at
//! least one of the standard accuracy anomalies (negative
//! notional, negative valuation, zero notional, placeholder /
//! abnormal maturity date).
//!
//! Multi-field rollup of the per-row `EMIR.ACC.*` granular
//! check family. Lets the committee see "how dirty is the
//! TSR overall on accuracy" in one number, vs scanning ~15
//! different per-row check IDs in the issues stream.
//!
//! - **Layer:** TSR.
//! - **Denominator:** all TSR records.
//! - **Numerator:** records exhibiting ≥ 1 anomaly across the
//!   inspected fields.
//! - **Dimension:** accuracy.
//!
//! v0.16 inspects 4 anomaly classes : negative notional,
//! zero notional, negative valuation, placeholder maturity.
//! Extension to additional fields (price, margins, etc.) is
//! a follow-up.

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;

use crate::dq::dqi::compute::{resolve_threshold, EVIDENCE_TOP_N};
use crate::dq::dqi::{DqiEvidence, DqiIndicator, MappingPresence};
use crate::model::{DqDimension, Regime, TrStateRecord};
use crate::scoring::rate_with_status;
use crate::Thresholds;

const INDICATOR_ID: &str = "DQI_ANOMALY_RATE";
const DESCRIPTION: &str = "Share of TSR records exhibiting at least one accuracy anomaly \
(negative notional, zero notional, negative valuation, placeholder maturity). Multi-field \
rollup of EMIR.ACC.* granular checks.";

/// Standard placeholder maturity dates seen in production
/// reporting — values that mean "we don't really know".
fn is_placeholder_maturity(d: NaiveDate) -> bool {
    matches!(
        (d.year(), d.month(), d.day()),
        (1900, 1, 1) | (9999, 12, 31) | (2099, 12, 31)
    )
}

fn record_anomalies(r: &TrStateRecord) -> Vec<&'static str> {
    let mut a = Vec::new();
    if let Some(n) = &r.notional_amount {
        if *n < Decimal::ZERO {
            a.push("NEGATIVE_NOTIONAL");
        } else if *n == Decimal::ZERO {
            a.push("ZERO_NOTIONAL");
        }
    }
    if let Some(v) = &r.valuation_amount {
        if *v < Decimal::ZERO {
            a.push("NEGATIVE_VALUATION");
        }
    }
    if let Some(m) = r.maturity_date {
        if is_placeholder_maturity(m) {
            a.push("PLACEHOLDER_MATURITY");
        }
    }
    a
}

/// Compute `DQI_ANOMALY_RATE`.
pub fn compute_dqi_anomaly_rate(
    tsr: &[TrStateRecord],
    thresholds: &Thresholds,
    _mapping_presence: MappingPresence,
) -> (DqiIndicator, Vec<DqiEvidence>) {
    let mut denominator: u64 = 0;
    let mut numerator: u64 = 0;
    let mut offenders: Vec<DqiEvidence> = Vec::new();

    for r in tsr {
        denominator += 1;
        let anomalies = record_anomalies(r);
        if anomalies.is_empty() {
            continue;
        }
        numerator += 1;
        offenders.push(DqiEvidence {
            indicator_id: INDICATOR_ID.into(),
            uti: r.uti.clone().unwrap_or_default(),
            counterparty: r.reporting_counterparty.clone(),
            asset_class: None,
            source_file: r.source_file.clone(),
            observed_value: Some(anomalies.join(",")),
            explanation: format!(
                "{} anomaly class(es): {}",
                anomalies.len(),
                anomalies.join(", ")
            ),
        });
    }

    offenders.sort_by(|a, b| {
        a.source_file
            .cmp(&b.source_file)
            .then_with(|| a.uti.cmp(&b.uti))
    });
    offenders.truncate(EVIDENCE_TOP_N);

    let pair = resolve_threshold(thresholds, INDICATOR_ID);
    let (rate, status) = rate_with_status(numerator, denominator, &pair);
    let indicator = DqiIndicator {
        indicator_id: INDICATOR_ID.into(),
        regime: Regime::Emir,
        dimension: DqDimension::Accuracy,
        table_scope: "TSR".into(),
        numerator,
        denominator,
        rate,
        threshold_amber: Some(pair.amber),
        threshold_red: Some(pair.red),
        status,
        description: DESCRIPTION.into(),
    };
    (indicator, offenders)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dq::dqi::DqiStatus;

    fn rec(uti: &str, n: Option<i64>, v: Option<i64>, m: Option<NaiveDate>) -> TrStateRecord {
        TrStateRecord {
            uti: Some(uti.into()),
            notional_amount: n.map(Decimal::from),
            valuation_amount: v.map(Decimal::from),
            maturity_date: m,
            ..Default::default()
        }
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn empty_is_not_applicable() {
        let (ind, _) =
            compute_dqi_anomaly_rate(&[], &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.status, DqiStatus::NotApplicable);
    }

    #[test]
    fn clean_records_green() {
        let recs = vec![
            rec("U1", Some(1_000_000), Some(50_000), Some(d(2028, 6, 30))),
            rec("U2", Some(2_000_000), Some(100_000), Some(d(2029, 3, 15))),
        ];
        let (ind, _) =
            compute_dqi_anomaly_rate(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 0);
        assert_eq!(ind.status, DqiStatus::Green);
    }

    #[test]
    fn negative_notional_fires() {
        let recs = vec![rec("U1", Some(-1000), None, None)];
        let (ind, ev) =
            compute_dqi_anomaly_rate(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 1);
        assert!(ev[0]
            .observed_value
            .as_deref()
            .unwrap()
            .contains("NEGATIVE_NOTIONAL"));
    }

    #[test]
    fn placeholder_maturity_fires() {
        let recs = vec![rec("U1", Some(1000), None, Some(d(9999, 12, 31)))];
        let (ind, _) =
            compute_dqi_anomaly_rate(&recs, &Thresholds::default(), MappingPresence::default());
        assert_eq!(ind.numerator, 1);
    }

    #[test]
    fn multiple_anomalies_count_one_record() {
        let recs = vec![rec("U1", Some(0), Some(-100), Some(d(1900, 1, 1)))];
        let (ind, ev) =
            compute_dqi_anomaly_rate(&recs, &Thresholds::default(), MappingPresence::default());
        // One record fires (numerator = 1), but evidence lists 3 anomaly classes.
        assert_eq!(ind.numerator, 1);
        let obs = ev[0].observed_value.as_deref().unwrap();
        assert!(obs.contains("ZERO_NOTIONAL"));
        assert!(obs.contains("NEGATIVE_VALUATION"));
        assert!(obs.contains("PLACEHOLDER_MATURITY"));
    }
}
