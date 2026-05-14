//! TR Reconciliation Statistics ingestion — ISO 20022 `auth.091` for
//! EMIR. The official SWIFT-licensed XSD is not redistributed; the
//! adapter parses a plausible structure aligned with public ISO
//! 20022 catalog conventions.
//!
//! Each record summarises pairing / reconciliation rates for one
//! reporting period and counterparty — TR-produced statistical
//! feedback distinct from the per-trade `auth.106` reconciliation
//! report. See `docs/auth-messages.md` for the canonical message
//! catalog.

use std::path::Path;
use std::str::FromStr;

use chrono::NaiveDate;
use opendqi_core::{DqDimension, DqIssue, ReconStatsRecord, Regime, Severity};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use rust_decimal::Decimal;
use tracing::warn;

use crate::wellformed::check_wellformedness;

/// Outcome of reading one auth.091 XML file.
#[derive(Debug, Default)]
pub struct ReconStatsXmlReadOutcome {
    /// Records extracted from the file.
    pub records: Vec<ReconStatsRecord>,
    /// File-level data-quality issues (format / namespace).
    pub issues: Vec<DqIssue>,
}

const ISO20022_AUTH_091_NS: &[u8] = b"urn:iso:std:iso:20022:tech:xsd:auth.091.001.01";
const RECON_STAT_BLOCK: &str = "ReconStat";

/// Read an EMIR `auth.091` Reconciliation Statistics file.
pub fn read_emir_recon_stats_xml(path: &Path) -> anyhow::Result<ReconStatsXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();

    if let Err(err) = check_wellformedness(path) {
        return Ok(ReconStatsXmlReadOutcome {
            records: vec![],
            issues: vec![DqIssue {
                check_id: "EMIR.FMT.XML_NOT_WELLFORMED".into(),
                regime: Regime::Emir,
                severity: Severity::Critical,
                dimension: DqDimension::Validity,
                record_id: None,
                uti: None,
                field: None,
                value: None,
                message: format!("XML is not well-formed: {}", err.message),
                source_file: Some(source_label),
            }],
        });
    }

    match peek_root_namespace(path)? {
        Some(ns) if ns == ISO20022_AUTH_091_NS => parse(path),
        other => {
            let actual = other
                .as_deref()
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_else(|| "(none)".into());
            Ok(ReconStatsXmlReadOutcome {
                records: vec![],
                issues: vec![DqIssue {
                    check_id: "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE".into(),
                    regime: Regime::Emir,
                    severity: Severity::Warning,
                    dimension: DqDimension::Validity,
                    record_id: None,
                    uti: None,
                    field: None,
                    value: None,
                    message: format!(
                        "Root namespace is '{actual}', expected 'urn:iso:std:iso:20022:tech:xsd:auth.091.001.01'."
                    ),
                    source_file: Some(source_label),
                }],
            })
        }
    }
}

fn peek_root_namespace(path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (ns, Event::Start(_)) | (ns, Event::Empty(_)) => {
                return Ok(match ns {
                    ResolveResult::Bound(n) => Some(n.as_ref().to_owned()),
                    _ => None,
                });
            }
            (_, Event::Eof) => return Ok(None),
            _ => {}
        }
        buf.clear();
    }
}

fn parse(path: &Path) -> anyhow::Result<ReconStatsXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();

    let mut current: Option<ReconStatsRecord> = None;
    let mut rec_depth: Option<usize> = None;
    let mut records: Vec<ReconStatsRecord> = Vec::new();
    let mut rec_index: u32 = 0;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local);
                text_buf.clear();

                if current.is_none() && pile.last().map(String::as_str) == Some(RECON_STAT_BLOCK) {
                    rec_index += 1;
                    current = Some(ReconStatsRecord {
                        source_file: Some(source_label.clone()),
                        record_id: Some(format!("{source_label}#rstat-{rec_index}")),
                        regime: Regime::Emir,
                        ..Default::default()
                    });
                    rec_depth = Some(pile.len());
                }
            }
            (_, Event::Empty(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local);
                pop_element(&mut pile, &mut is_leaf);
                text_buf.clear();
            }
            (_, Event::Text(t)) => {
                if let Ok(s) = t.unescape() {
                    text_buf.push_str(&s);
                }
            }
            (_, Event::CData(t)) => {
                if let Ok(s) = std::str::from_utf8(t.as_ref()) {
                    text_buf.push_str(s);
                }
            }
            (_, Event::End(_)) => {
                let leaf_now = is_leaf.last().copied().unwrap_or(false);
                if leaf_now {
                    let trimmed = text_buf.trim();
                    if let (Some(rec), Some(rdepth)) = (current.as_mut(), rec_depth) {
                        if pile.len() > rdepth {
                            commit_leaf(rec, &pile[rdepth..], trimmed);
                        }
                    }
                }

                if let Some(rdepth) = rec_depth {
                    if pile.len() == rdepth {
                        if let Some(rec) = current.take() {
                            records.push(rec);
                        }
                        rec_depth = None;
                    }
                }

                pop_element(&mut pile, &mut is_leaf);
                text_buf.clear();
            }
            (_, Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(ReconStatsXmlReadOutcome {
        records,
        issues: Vec::new(),
    })
}

fn commit_leaf(rec: &mut ReconStatsRecord, rel: &[String], value: &str) {
    if value.is_empty() {
        return;
    }
    let path: Vec<&str> = rel.iter().map(String::as_str).collect();
    let record_id = rec.record_id.clone().unwrap_or_default();
    match path.as_slice() {
        ["RptgDt"] | ["ReportingDate"] => set_date(&mut rec.reporting_date, value, &record_id),
        ["CtrPty", "LEI"] | ["Cpty", "LEI"] | ["CounterpartyLEI"] => {
            rec.counterparty_lei = Some(value.to_owned())
        }
        ["PairgRate"] | ["PairingRate"] => set_decimal(&mut rec.pairing_rate, value, &record_id),
        ["RcncltnRate"] | ["RecnRate"] | ["ReconciliationRate"] => {
            set_decimal(&mut rec.recon_rate, value, &record_id)
        }
        ["OutsdngPaired"] | ["OutstandingPaired"] => {
            set_i64(&mut rec.outstanding_paired, value, &record_id)
        }
        ["OutsdngUnpaired"] | ["OutstandingUnpaired"] => {
            set_i64(&mut rec.outstanding_unpaired, value, &record_id)
        }
        _ => {
            let key = rel.join("/");
            rec.raw_fields.insert(key, value.to_owned());
        }
    }
}

fn set_decimal(slot: &mut Option<Decimal>, value: &str, record_id: &str) {
    match Decimal::from_str(value) {
        Ok(v) => *slot = Some(v),
        Err(err) => warn!(record = %record_id, value, %err, "auth.091 decimal parse failed"),
    }
}

fn set_i64(slot: &mut Option<i64>, value: &str, record_id: &str) {
    match value.parse::<i64>() {
        Ok(v) => *slot = Some(v),
        Err(err) => warn!(record = %record_id, value, %err, "auth.091 integer parse failed"),
    }
}

fn set_date(slot: &mut Option<NaiveDate>, value: &str, record_id: &str) {
    match NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        Ok(v) => *slot = Some(v),
        Err(err) => warn!(record = %record_id, value, %err, "auth.091 date parse failed"),
    }
}

fn push_element(pile: &mut Vec<String>, is_leaf: &mut Vec<bool>, local: String) {
    pile.push(local);
    is_leaf.push(true);
    let n = is_leaf.len();
    if n >= 2 {
        is_leaf[n - 2] = false;
    }
}

fn pop_element(pile: &mut Vec<String>, is_leaf: &mut Vec<bool>) {
    pile.pop();
    is_leaf.pop();
}

fn local_name(e: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(e.local_name().as_ref()).into_owned()
}
