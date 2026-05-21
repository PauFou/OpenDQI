//! `indicators.csv` + `evidence.csv` writers for the v0.15
//! Data Quality Pack outputs.
//!
//! Column orders are **v1.0 stable** — same contract as
//! `issues.csv`. A parity test in `opendqi-py` keeps the
//! Python Arrow schema in lockstep.

use std::path::Path;

use anyhow::{Context, Result};
use opendqi_core::{DqiEvidence, DqiIndicator};

/// Write the indicators to a CSV with a stable column order
/// (`indicators.csv`).
///
/// Rows are written in input order — the caller (the v0.15
/// orchestrator) already returns them sorted by
/// `indicator_id` ascending.
pub fn write_indicators_csv(path: &Path, indicators: &[DqiIndicator]) -> Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_path(path)
        .with_context(|| format!("creating {}", path.display()))?;

    writer.write_record([
        "indicator_id",
        "regime",
        "dimension",
        "table_scope",
        "numerator",
        "denominator",
        "rate",
        "threshold_amber",
        "threshold_red",
        "status",
        "description",
    ])?;

    for ind in indicators {
        let rate = ind.rate.map(|r| format!("{r:.6}")).unwrap_or_default();
        let amber = ind
            .threshold_amber
            .map(|t| format!("{t:.6}"))
            .unwrap_or_default();
        let red = ind
            .threshold_red
            .map(|t| format!("{t:.6}"))
            .unwrap_or_default();
        writer.write_record([
            ind.indicator_id.as_str(),
            &ind.regime.to_string(),
            &ind.dimension.to_string(),
            ind.table_scope.as_str(),
            &ind.numerator.to_string(),
            &ind.denominator.to_string(),
            rate.as_str(),
            amber.as_str(),
            red.as_str(),
            &ind.status.to_string(),
            ind.description.as_str(),
        ])?;
    }
    writer.flush()?;
    Ok(())
}

/// Write the evidence rows to `evidence.csv`.
///
/// Sorted by `(indicator_id, uti)` for reproducibility.
pub fn write_evidence_csv(path: &Path, evidence: &[DqiEvidence]) -> Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_path(path)
        .with_context(|| format!("creating {}", path.display()))?;

    writer.write_record([
        "indicator_id",
        "uti",
        "counterparty",
        "asset_class",
        "source_file",
        "observed_value",
        "explanation",
    ])?;

    let mut sorted: Vec<&DqiEvidence> = evidence.iter().collect();
    sorted.sort_by(|a, b| {
        a.indicator_id
            .cmp(&b.indicator_id)
            .then_with(|| a.uti.cmp(&b.uti))
    });

    for ev in sorted {
        writer.write_record([
            ev.indicator_id.as_str(),
            ev.uti.as_str(),
            ev.counterparty.as_deref().unwrap_or(""),
            ev.asset_class.as_deref().unwrap_or(""),
            ev.source_file.as_deref().unwrap_or(""),
            ev.observed_value.as_deref().unwrap_or(""),
            ev.explanation.as_str(),
        ])?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendqi_core::{DqDimension, DqiStatus, Regime};

    fn ind(id: &str, num: u64, denom: u64, status: DqiStatus, rate: Option<f64>) -> DqiIndicator {
        DqiIndicator {
            indicator_id: id.into(),
            regime: Regime::Emir,
            dimension: DqDimension::Completeness,
            table_scope: "TSR".into(),
            numerator: num,
            denominator: denom,
            rate,
            threshold_amber: Some(0.01),
            threshold_red: Some(0.05),
            status,
            description: "desc".into(),
        }
    }

    fn ev(id: &str, uti: &str) -> DqiEvidence {
        DqiEvidence {
            indicator_id: id.into(),
            uti: uti.into(),
            counterparty: None,
            asset_class: None,
            source_file: Some("tsr.xml".into()),
            observed_value: None,
            explanation: "exp".into(),
        }
    }

    #[test]
    fn indicators_csv_round_trip() {
        let path =
            std::env::temp_dir().join(format!("opendqi-indicators-{}.csv", std::process::id()));
        let rows = vec![
            ind("DQI_VAL_MISSING", 5, 100, DqiStatus::Amber, Some(0.05)),
            ind("DQI_VAL_STALE", 0, 0, DqiStatus::NotApplicable, None),
        ];
        write_indicators_csv(&path, &rows).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("indicator_id,regime,dimension,table_scope,numerator"));
        assert!(text.contains("DQI_VAL_MISSING,emir,completeness,TSR,5,100,0.050000"));
        assert!(text.contains(",not_applicable,desc"));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn evidence_csv_sorted_by_indicator_then_uti() {
        let path =
            std::env::temp_dir().join(format!("opendqi-evidence-{}.csv", std::process::id()));
        let rows = vec![
            ev("DQI_VAL_STALE", "U9"),
            ev("DQI_VAL_MISSING", "U2"),
            ev("DQI_VAL_MISSING", "U1"),
        ];
        write_evidence_csv(&path, &rows).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // header + 3 rows
        assert_eq!(lines.len(), 4);
        // First data row = DQI_VAL_MISSING, U1
        assert!(lines[1].starts_with("DQI_VAL_MISSING,U1,"));
        assert!(lines[2].starts_with("DQI_VAL_MISSING,U2,"));
        assert!(lines[3].starts_with("DQI_VAL_STALE,U9,"));
        std::fs::remove_file(&path).unwrap();
    }
}
