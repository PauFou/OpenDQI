//! YAML mapping from CSV columns to canonical EMIR fields.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Mapping description.
///
/// `fields` keys are canonical EMIR record field names (snake_case);
/// values are the column header names found in the source CSV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvMapping {
    /// Canonical → source-header mapping.
    pub fields: BTreeMap<String, String>,
    /// Format string for `NaiveDate` fields. Defaults to `%Y-%m-%d`.
    #[serde(default)]
    pub date_format: Option<String>,
    /// Format string for `DateTime<Utc>` fields. Defaults to RFC 3339.
    #[serde(default)]
    pub datetime_format: Option<String>,
}

impl CsvMapping {
    /// Read and parse a YAML mapping file.
    pub fn from_path(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading mapping file {}", path.display()))?;
        let m: CsvMapping = serde_yaml::from_str(&text)
            .with_context(|| format!("parsing mapping YAML {}", path.display()))?;
        Ok(m)
    }

    /// Effective date format (with the documented default).
    pub fn effective_date_format(&self) -> &str {
        self.date_format.as_deref().unwrap_or("%Y-%m-%d")
    }

    /// Effective datetime format (with the documented default).
    pub fn effective_datetime_format(&self) -> &str {
        self.datetime_format
            .as_deref()
            .unwrap_or("%Y-%m-%dT%H:%M:%S%.fZ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_mapping() {
        let yaml = "fields:\n  uti: trade_uti\n  notional_amount: notional\n";
        let m: CsvMapping = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(m.fields.get("uti").map(String::as_str), Some("trade_uti"));
        assert_eq!(m.effective_date_format(), "%Y-%m-%d");
    }
}
