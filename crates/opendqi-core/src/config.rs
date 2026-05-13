//! Configurable thresholds used by data-quality checks.
//!
//! Defaults are hardcoded in [`Thresholds::default`] so that an MVP
//! scan can run without a config file. A YAML file passed via
//! `--config` overrides any subset of the defaults.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Top-level threshold configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Thresholds {
    /// Timeliness limits.
    pub timeliness: TimelinessThresholds,
    /// Maturity-date sanity bounds.
    pub maturity: MaturityThresholds,
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
}
