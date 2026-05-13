//! EMIR.UNI.DUPLICATE_UTI — UTIs identify trades uniquely.

use std::collections::BTreeMap;

use super::{Check, CheckContext};
use crate::model::{DqDimension, DqIssue, EmirRecord, Regime, Severity};

/// Check implementation.
pub struct DuplicateUti;

const CHECK_ID: &str = "EMIR.UNI.DUPLICATE_UTI";

impl Check for DuplicateUti {
    fn id(&self) -> &'static str {
        CHECK_ID
    }
    fn dimension(&self) -> DqDimension {
        DqDimension::Uniqueness
    }
    fn severity(&self) -> Severity {
        Severity::Critical
    }
    fn run(&self, records: &[EmirRecord], _ctx: &CheckContext) -> Vec<DqIssue> {
        let mut by_uti: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (idx, r) in records.iter().enumerate() {
            if let Some(uti) = r.uti.as_deref() {
                let trimmed = uti.trim();
                if !trimmed.is_empty() {
                    by_uti.entry(trimmed).or_default().push(idx);
                }
            }
        }

        let mut out = Vec::new();
        for (uti, indices) in by_uti {
            if indices.len() <= 1 {
                continue;
            }
            let sources: Vec<&str> = indices
                .iter()
                .filter_map(|&i| records[i].source_file.as_deref())
                .collect();
            let summary = if sources.is_empty() {
                String::new()
            } else {
                format!(" Sources: {}.", sources.join(", "))
            };
            for &idx in &indices {
                let r = &records[idx];
                out.push(DqIssue {
                    check_id: CHECK_ID.into(),
                    regime: Regime::Emir,
                    severity: Severity::Critical,
                    dimension: DqDimension::Uniqueness,
                    record_id: r.record_id.clone(),
                    uti: Some(uti.to_string()),
                    field: Some("uti".into()),
                    value: Some(uti.to_string()),
                    message: format!(
                        "UTI {uti} appears {n} times in active records.{summary}",
                        n = indices.len()
                    ),
                    source_file: r.source_file.clone(),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_duplicate_uti() {
        let records = vec![
            EmirRecord {
                uti: Some("X".into()),
                record_id: Some("1".into()),
                ..Default::default()
            },
            EmirRecord {
                uti: Some("X".into()),
                record_id: Some("2".into()),
                ..Default::default()
            },
            EmirRecord {
                uti: Some("Y".into()),
                record_id: Some("3".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = DuplicateUti.run(&records, &ctx);
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn empty_or_missing_uti_is_not_counted() {
        let records = vec![
            EmirRecord {
                uti: None,
                record_id: Some("1".into()),
                ..Default::default()
            },
            EmirRecord {
                uti: Some("".into()),
                record_id: Some("2".into()),
                ..Default::default()
            },
            EmirRecord {
                uti: Some(" ".into()),
                record_id: Some("3".into()),
                ..Default::default()
            },
        ];
        let ctx = CheckContext::now_with_defaults();
        let issues = DuplicateUti.run(&records, &ctx);
        assert!(issues.is_empty());
    }
}
