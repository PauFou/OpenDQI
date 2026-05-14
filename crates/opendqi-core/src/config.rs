//! Configurable thresholds used by data-quality checks.
//!
//! Defaults are hardcoded in [`Thresholds::default`] so that an MVP
//! scan can run without a config file. A YAML file passed via
//! `--config` overrides any subset of the defaults.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::model::Severity;

/// Top-level threshold configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Thresholds {
    /// Timeliness limits.
    pub timeliness: TimelinessThresholds,
    /// Maturity-date sanity bounds.
    pub maturity: MaturityThresholds,
    /// EMIR Article 11 risk-mitigation thresholds (per asset class).
    pub emir_rmt: EmirRmtThresholds,
    /// Per-check-id severity overrides. Keys are full check IDs
    /// (e.g. `EMIR.COMP.UTI_MISSING`); values replace the check's
    /// default severity at issue-emission time. Empty by default.
    pub severity_overrides: BTreeMap<String, Severity>,
}

/// Timeliness configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimelinessThresholds {
    /// Maximum delay, in hours, between event and reporting timestamp.
    pub max_reporting_delay_hours: i64,
    /// Maximum age, in business days, of a valuation timestamp.
    pub max_valuation_age_business_days: i64,
}

/// Maturity-date configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MaturityThresholds {
    /// Maturity dates farther than this number of years in the future
    /// are treated as abnormal.
    pub abnormal_maturity_years: i32,
    /// Hard-coded placeholder dates (e.g. `9999-12-31`).
    pub placeholder_dates: Vec<NaiveDate>,
}

/// EMIR Article 11 risk-mitigation thresholds. Each NFC clearing
/// threshold is keyed by the canonical asset-class short code (see
/// [`crate::dq::formats::canonical_asset_class`]).
///
/// YAML semantics: each scalar field falls back to its ESMA default
/// when omitted. The NFC map is treated as a whole — providing it in
/// YAML replaces the entire map, so the caller must list every class
/// they want to keep. Omitting the `nfc_clearing_thresholds_eur` key
/// (or the whole `emir_rmt` section) keeps the ESMA defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EmirRmtThresholds {
    /// NFC clearing thresholds (EMIR Article 10) per canonical asset
    /// class code: `IR`, `CR`, `EQ`, `FX`, `CO`. Value = notional
    /// threshold in EUR.
    #[serde(default = "default_nfc_clearing_thresholds_eur")]
    pub nfc_clearing_thresholds_eur: BTreeMap<String, i64>,
    /// AANA initial-margin threshold (EMIR Article 11 RTS) in EUR.
    /// Defaults to the phase 6 value (8 G€); update via YAML when a
    /// new phase takes effect.
    #[serde(default = "default_aana_im_threshold_eur")]
    pub aana_im_threshold_eur: i64,
}

fn default_nfc_clearing_thresholds_eur() -> BTreeMap<String, i64> {
    let mut nfc = BTreeMap::new();
    nfc.insert("CR".into(), 1_000_000_000);
    nfc.insert("EQ".into(), 1_000_000_000);
    nfc.insert("IR".into(), 3_000_000_000);
    nfc.insert("FX".into(), 3_000_000_000);
    nfc.insert("CO".into(), 4_000_000_000);
    nfc
}

fn default_aana_im_threshold_eur() -> i64 {
    8_000_000_000
}

impl Default for EmirRmtThresholds {
    fn default() -> Self {
        Self {
            nfc_clearing_thresholds_eur: default_nfc_clearing_thresholds_eur(),
            aana_im_threshold_eur: default_aana_im_threshold_eur(),
        }
    }
}

impl Default for TimelinessThresholds {
    fn default() -> Self {
        Self {
            max_reporting_delay_hours: 24,
            max_valuation_age_business_days: 1,
        }
    }
}

impl Default for MaturityThresholds {
    fn default() -> Self {
        Self {
            abnormal_maturity_years: 51,
            placeholder_dates: vec![
                NaiveDate::from_ymd_opt(1900, 1, 1).expect("static date"),
                NaiveDate::from_ymd_opt(2099, 12, 31).expect("static date"),
                NaiveDate::from_ymd_opt(9999, 12, 31).expect("static date"),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_reasonable() {
        let t = Thresholds::default();
        assert_eq!(t.timeliness.max_reporting_delay_hours, 24);
        assert_eq!(t.maturity.abnormal_maturity_years, 51);
        assert_eq!(t.maturity.placeholder_dates.len(), 3);
    }

    #[test]
    fn yaml_partial_override_keeps_other_defaults() {
        let yaml = "timeliness:\n  max_reporting_delay_hours: 48\n";
        let t: Thresholds = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(t.timeliness.max_reporting_delay_hours, 48);
        // untouched section still uses defaults
        assert_eq!(t.maturity.abnormal_maturity_years, 51);
    }

    #[test]
    fn emir_rmt_defaults_match_esma_values() {
        let t = Thresholds::default();
        let nfc = &t.emir_rmt.nfc_clearing_thresholds_eur;
        assert_eq!(nfc.get("CR"), Some(&1_000_000_000));
        assert_eq!(nfc.get("EQ"), Some(&1_000_000_000));
        assert_eq!(nfc.get("IR"), Some(&3_000_000_000));
        assert_eq!(nfc.get("FX"), Some(&3_000_000_000));
        assert_eq!(nfc.get("CO"), Some(&4_000_000_000));
        assert_eq!(t.emir_rmt.aana_im_threshold_eur, 8_000_000_000);
    }

    #[test]
    fn emir_rmt_yaml_replaces_nfc_map_wholesale() {
        // When the nfc_clearing_thresholds_eur key is provided, the
        // map is replaced entirely. AANA omitted → falls back to its
        // default via `#[serde(default = ...)]`.
        let yaml = "emir_rmt:\n  nfc_clearing_thresholds_eur:\n    IR: 5000000000\n";
        let t: Thresholds = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            t.emir_rmt.nfc_clearing_thresholds_eur.get("IR"),
            Some(&5_000_000_000)
        );
        assert_eq!(t.emir_rmt.nfc_clearing_thresholds_eur.get("CR"), None);
        // omitted scalar field falls back to its ESMA default
        assert_eq!(t.emir_rmt.aana_im_threshold_eur, 8_000_000_000);
        // unrelated section untouched
        assert_eq!(t.timeliness.max_reporting_delay_hours, 24);
    }

    #[test]
    fn severity_overrides_round_trip_via_yaml() {
        let yaml = "severity_overrides:\n  EMIR.COMP.UTI_MISSING: warning\n  EMIR.UNI.DUPLICATE_UTI: critical\n";
        let t: Thresholds = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            t.severity_overrides.get("EMIR.COMP.UTI_MISSING"),
            Some(&crate::model::Severity::Warning)
        );
        assert_eq!(
            t.severity_overrides.get("EMIR.UNI.DUPLICATE_UTI"),
            Some(&crate::model::Severity::Critical)
        );
        // Unaffected sections still default.
        assert_eq!(t.timeliness.max_reporting_delay_hours, 24);
    }

    #[test]
    fn emir_rmt_yaml_aana_only_keeps_nfc_defaults() {
        // Providing only the AANA override leaves the NFC map at its
        // ESMA defaults via the per-field `#[serde(default = ...)]`.
        let yaml = "emir_rmt:\n  aana_im_threshold_eur: 50000000000\n";
        let t: Thresholds = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(t.emir_rmt.aana_im_threshold_eur, 50_000_000_000);
        assert_eq!(
            t.emir_rmt.nfc_clearing_thresholds_eur.get("CR"),
            Some(&1_000_000_000)
        );
        assert_eq!(
            t.emir_rmt.nfc_clearing_thresholds_eur.get("IR"),
            Some(&3_000_000_000)
        );
    }
}
