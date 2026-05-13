//! Extract `EmirRecord` values from the simplified OpenDQI EMIR XML format.
//!
//! See `docs/xml-format.md` for the schema specification. Briefly:
//! a `<EmirReport>` root contains an optional `<Header>` (whose values
//! act as defaults) and zero or more `<Trade>` elements.

use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::{DqDimension, DqIssue, EmirRecord, Regime, Severity};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use rust_decimal::Decimal;
use tracing::warn;

use super::XmlReadOutcome;

const EXPECTED_NAMESPACE: &[u8] = b"https://opendqi.org/schemas/emir/v0.1";

const CHECK_UNSUPPORTED_NS: &str = "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE";
const CHECK_UNKNOWN_ELEMENT: &str = "EMIR.FMT.XML_UNKNOWN_ELEMENT";

/// Read a single well-formed EMIR XML file in the simplified v0.1 format.
pub fn read(path: &Path) -> anyhow::Result<XmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut state = State::Root;
    let mut current_field: Option<String> = None;
    let mut current_text = String::new();
    let mut current_attrs: Vec<(String, String)> = Vec::new();

    let mut header_err_lei: Option<String> = None;
    let mut header_report_ts: Option<DateTime<Utc>> = None;

    let mut current_record: Option<EmirRecord> = None;
    let mut records: Vec<EmirRecord> = Vec::new();
    let mut issues: Vec<DqIssue> = Vec::new();
    let mut unknown_elements: BTreeSet<String> = BTreeSet::new();
    let mut namespace_issue_emitted = false;
    let mut record_index: usize = 0;

    let mut buf = Vec::new();
    loop {
        let event = reader.read_resolved_event_into(&mut buf)?;
        match event {
            (ns, Event::Start(e)) => {
                let local = local_name(&e);
                match state {
                    State::Root => {
                        if local == "EmirReport" {
                            check_root_namespace(
                                &ns,
                                &source_label,
                                &mut issues,
                                &mut namespace_issue_emitted,
                            );
                            state = State::InReport;
                        } else {
                            push_unknown(&local, &mut unknown_elements, &source_label, &mut issues);
                        }
                    }
                    State::InReport => match local.as_str() {
                        "Header" => state = State::InHeader,
                        "Trade" => {
                            record_index += 1;
                            current_record = Some(new_trade(
                                &source_label,
                                record_index,
                                header_err_lei.as_deref(),
                                header_report_ts,
                            ));
                            state = State::InTrade;
                        }
                        _ => {
                            push_unknown(&local, &mut unknown_elements, &source_label, &mut issues)
                        }
                    },
                    State::InHeader => {
                        if !is_known_header_field(&local) {
                            push_unknown(&local, &mut unknown_elements, &source_label, &mut issues);
                        }
                        current_field = Some(local);
                        current_text.clear();
                        state = State::InHeaderField;
                    }
                    State::InTrade => {
                        if !is_known_trade_field(&local) {
                            push_unknown(&local, &mut unknown_elements, &source_label, &mut issues);
                        }
                        current_field = Some(local);
                        current_text.clear();
                        current_attrs = capture_attrs(&e);
                        state = State::InTradeField;
                    }
                    State::InHeaderField | State::InTradeField => {
                        push_unknown(&local, &mut unknown_elements, &source_label, &mut issues);
                    }
                }
            }
            (ns, Event::Empty(e)) => {
                let local = local_name(&e);
                let attrs = capture_attrs(&e);
                match state {
                    State::Root => {
                        if local == "EmirReport" {
                            check_root_namespace(
                                &ns,
                                &source_label,
                                &mut issues,
                                &mut namespace_issue_emitted,
                            );
                            // immediately closed: no records
                        } else {
                            push_unknown(&local, &mut unknown_elements, &source_label, &mut issues);
                        }
                    }
                    State::InReport => match local.as_str() {
                        "Header" => {}
                        "Trade" => {
                            record_index += 1;
                            records.push(new_trade(
                                &source_label,
                                record_index,
                                header_err_lei.as_deref(),
                                header_report_ts,
                            ));
                        }
                        _ => {
                            push_unknown(&local, &mut unknown_elements, &source_label, &mut issues)
                        }
                    },
                    State::InHeader => {
                        if !is_known_header_field(&local) {
                            push_unknown(&local, &mut unknown_elements, &source_label, &mut issues);
                        }
                        commit_header_field(&local, "", &mut header_err_lei, &mut header_report_ts);
                    }
                    State::InTrade => {
                        if !is_known_trade_field(&local) {
                            push_unknown(&local, &mut unknown_elements, &source_label, &mut issues);
                        }
                        if let Some(rec) = current_record.as_mut() {
                            commit_trade_field(&local, "", &attrs, rec);
                        }
                    }
                    State::InHeaderField | State::InTradeField => {
                        push_unknown(&local, &mut unknown_elements, &source_label, &mut issues);
                    }
                }
            }
            (_, Event::Text(t)) => {
                if matches!(state, State::InHeaderField | State::InTradeField) {
                    if let Ok(s) = t.unescape() {
                        current_text.push_str(&s);
                    }
                }
            }
            (_, Event::CData(t)) => {
                if matches!(state, State::InHeaderField | State::InTradeField) {
                    if let Ok(s) = std::str::from_utf8(t.as_ref()) {
                        current_text.push_str(s);
                    }
                }
            }
            (_, Event::End(_)) => match state {
                State::InHeaderField => {
                    if let Some(name) = current_field.take() {
                        commit_header_field(
                            &name,
                            &current_text,
                            &mut header_err_lei,
                            &mut header_report_ts,
                        );
                    }
                    current_text.clear();
                    state = State::InHeader;
                }
                State::InTradeField => {
                    if let Some(name) = current_field.take() {
                        if let Some(rec) = current_record.as_mut() {
                            commit_trade_field(&name, &current_text, &current_attrs, rec);
                        }
                    }
                    current_text.clear();
                    current_attrs.clear();
                    state = State::InTrade;
                }
                State::InHeader => state = State::InReport,
                State::InTrade => {
                    if let Some(rec) = current_record.take() {
                        records.push(rec);
                    }
                    state = State::InReport;
                }
                State::InReport => state = State::Root,
                State::Root => {}
            },
            (_, Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(XmlReadOutcome { records, issues })
}

#[derive(Debug)]
enum State {
    Root,
    InReport,
    InHeader,
    InHeaderField,
    InTrade,
    InTradeField,
}

fn local_name(e: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(e.local_name().as_ref()).into_owned()
}

fn is_known_header_field(name: &str) -> bool {
    matches!(name, "EntityResponsibleForReporting" | "ReportingTimestamp")
}

fn is_known_trade_field(name: &str) -> bool {
    matches!(
        name,
        "UTI"
            | "PriorUTI"
            | "ActionType"
            | "EventType"
            | "EntityResponsibleForReporting"
            | "Counterparty1"
            | "Counterparty2"
            | "AssetClass"
            | "ProductId"
            | "UnderlyingId"
            | "Notional"
            | "Price"
            | "ExecutionTimestamp"
            | "EventTimestamp"
            | "ReportingTimestamp"
            | "EffectiveDate"
            | "MaturityDate"
            | "TerminationDate"
            | "Valuation"
            | "CollateralPortfolioCode"
            | "ClearingStatus"
            | "CollateralisationCategory"
    )
}

fn capture_attrs(e: &BytesStart<'_>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for attr in e.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).into_owned();
        let value = attr
            .unescape_value()
            .map(|c| c.into_owned())
            .unwrap_or_default();
        out.push((key, value));
    }
    out
}

fn new_trade(
    source_label: &str,
    record_index: usize,
    header_err_lei: Option<&str>,
    header_report_ts: Option<DateTime<Utc>>,
) -> EmirRecord {
    EmirRecord {
        source_file: Some(source_label.to_owned()),
        record_id: Some(format!("{source_label}#trade-{record_index}")),
        entity_responsible_for_reporting: header_err_lei.map(str::to_owned),
        reporting_timestamp: header_report_ts,
        ..Default::default()
    }
}

fn check_root_namespace(
    ns: &ResolveResult<'_>,
    source_label: &str,
    issues: &mut Vec<DqIssue>,
    already_emitted: &mut bool,
) {
    if *already_emitted {
        return;
    }
    let matches = match ns {
        ResolveResult::Bound(n) => n.as_ref() == EXPECTED_NAMESPACE,
        _ => false,
    };
    if !matches {
        let actual = match ns {
            ResolveResult::Bound(n) => String::from_utf8_lossy(n.as_ref()).into_owned(),
            ResolveResult::Unbound => "(none)".into(),
            ResolveResult::Unknown(_) => "(unknown prefix)".into(),
        };
        issues.push(format_issue(
            CHECK_UNSUPPORTED_NS,
            Severity::Warning,
            format!(
                "Root namespace is '{actual}', expected '{}'. Extraction will proceed best-effort.",
                String::from_utf8_lossy(EXPECTED_NAMESPACE)
            ),
            source_label,
        ));
        *already_emitted = true;
    }
}

fn push_unknown(
    local: &str,
    unknown_elements: &mut BTreeSet<String>,
    source_label: &str,
    issues: &mut Vec<DqIssue>,
) {
    if unknown_elements.insert(local.to_owned()) {
        issues.push(DqIssue {
            check_id: CHECK_UNKNOWN_ELEMENT.into(),
            regime: Regime::Emir,
            severity: Severity::Info,
            dimension: DqDimension::Validity,
            record_id: None,
            uti: None,
            field: Some(local.to_owned()),
            value: None,
            message: format!(
                "Element <{local}> is not part of the OpenDQI EMIR v0.1 schema; ignored."
            ),
            source_file: Some(source_label.to_owned()),
        });
    }
}

fn format_issue(
    check_id: &str,
    severity: Severity,
    message: String,
    source_label: &str,
) -> DqIssue {
    DqIssue {
        check_id: check_id.into(),
        regime: Regime::Emir,
        severity,
        dimension: DqDimension::Validity,
        record_id: None,
        uti: None,
        field: None,
        value: None,
        message,
        source_file: Some(source_label.to_owned()),
    }
}

fn commit_header_field(
    name: &str,
    text: &str,
    err_lei: &mut Option<String>,
    report_ts: &mut Option<DateTime<Utc>>,
) {
    let trimmed = text.trim();
    match name {
        "EntityResponsibleForReporting" if !trimmed.is_empty() => {
            *err_lei = Some(trimmed.to_owned());
        }
        "ReportingTimestamp" => {
            *report_ts = parse_dt(trimmed, "ReportingTimestamp", "header");
        }
        _ => {}
    }
}

fn commit_trade_field(name: &str, text: &str, attrs: &[(String, String)], rec: &mut EmirRecord) {
    let trimmed = text.trim();
    let value = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    };
    let record_id = rec.record_id.clone().unwrap_or_default();

    match name {
        "UTI" => rec.uti = value.map(str::to_owned),
        "PriorUTI" => rec.prior_uti = value.map(str::to_owned),
        "ActionType" => rec.action_type = value.map(str::to_owned),
        "EventType" => rec.event_type = value.map(str::to_owned),
        "EntityResponsibleForReporting" if value.is_some() => {
            rec.entity_responsible_for_reporting = value.map(str::to_owned);
        }
        "Counterparty1" => rec.counterparty_1 = value.map(str::to_owned),
        "Counterparty2" => rec.counterparty_2 = value.map(str::to_owned),
        "AssetClass" => rec.asset_class = value.map(str::to_owned),
        "ProductId" => rec.product_id = value.map(str::to_owned),
        "UnderlyingId" => rec.underlying_id = value.map(str::to_owned),
        "Notional" => {
            rec.notional_amount = value.and_then(|v| parse_decimal(v, name, &record_id));
            rec.notional_currency = attr(attrs, "currency");
        }
        "Price" => {
            rec.price = value.and_then(|v| parse_decimal(v, name, &record_id));
            rec.price_currency = attr(attrs, "currency");
        }
        "ExecutionTimestamp" => {
            rec.execution_timestamp = value.and_then(|v| parse_dt(v, name, &record_id));
        }
        "EventTimestamp" => {
            rec.event_timestamp = value.and_then(|v| parse_dt(v, name, &record_id));
        }
        "ReportingTimestamp" => {
            if let Some(v) = value {
                if let Some(dt) = parse_dt(v, name, &record_id) {
                    rec.reporting_timestamp = Some(dt);
                }
            }
        }
        "EffectiveDate" => {
            rec.effective_date = value.and_then(|v| parse_date(v, name, &record_id));
        }
        "MaturityDate" => {
            rec.maturity_date = value.and_then(|v| parse_date(v, name, &record_id));
        }
        "TerminationDate" => {
            rec.termination_date = value.and_then(|v| parse_date(v, name, &record_id));
        }
        "Valuation" => {
            rec.valuation_amount = value.and_then(|v| parse_decimal(v, name, &record_id));
            rec.valuation_currency = attr(attrs, "currency");
            rec.valuation_timestamp = attr(attrs, "timestamp")
                .as_deref()
                .and_then(|v| parse_dt(v, "Valuation/@timestamp", &record_id));
        }
        "CollateralPortfolioCode" => {
            rec.collateral_portfolio_code = value.map(str::to_owned);
        }
        "ClearingStatus" => rec.clearing_status = value.map(str::to_owned),
        "CollateralisationCategory" => {
            rec.collateralisation_category = value.map(str::to_owned);
        }
        _ => {
            // Unknown trade-level element: the start handler already
            // logged it (we transitioned to InTradeField). Nothing to
            // commit.
        }
    }
}

fn attr(attrs: &[(String, String)], key: &str) -> Option<String> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

fn parse_decimal(v: &str, field: &str, record: &str) -> Option<Decimal> {
    match Decimal::from_str(v) {
        Ok(d) => Some(d),
        Err(e) => {
            warn!(record = %record, field, value = v, error = %e, "could not parse decimal");
            None
        }
    }
}

fn parse_date(v: &str, field: &str, record: &str) -> Option<NaiveDate> {
    match NaiveDate::parse_from_str(v, "%Y-%m-%d") {
        Ok(d) => Some(d),
        Err(e) => {
            warn!(record = %record, field, value = v, error = %e, "could not parse date");
            None
        }
    }
}

fn parse_dt(v: &str, field: &str, record: &str) -> Option<DateTime<Utc>> {
    match DateTime::parse_from_rfc3339(v) {
        Ok(dt) => Some(dt.with_timezone(&Utc)),
        Err(e) => {
            warn!(record = %record, field, value = v, error = %e, "could not parse datetime");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, content: &[u8]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("opendqi-xml-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn extracts_simple_trades() {
        let xml = br#"<?xml version="1.0"?>
<EmirReport xmlns="https://opendqi.org/schemas/emir/v0.1">
  <Header>
    <EntityResponsibleForReporting>DUMMYERR0000000000AA</EntityResponsibleForReporting>
    <ReportingTimestamp>2026-05-12T18:00:00Z</ReportingTimestamp>
  </Header>
  <Trade>
    <UTI>OPENDQI-XML-0001</UTI>
    <ActionType>NEWT</ActionType>
    <Notional currency="EUR">10000000</Notional>
    <MaturityDate>2031-02-02</MaturityDate>
    <Valuation currency="EUR" timestamp="2026-05-12T18:00:00Z">1500.50</Valuation>
  </Trade>
  <Trade>
    <UTI>OPENDQI-XML-0002</UTI>
    <ActionType>NEWT</ActionType>
    <Notional currency="USD">2500000</Notional>
  </Trade>
</EmirReport>
"#;
        let p = write_tmp("simple.xml", xml);
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();

        assert_eq!(out.records.len(), 2);
        assert_eq!(out.records[0].uti.as_deref(), Some("OPENDQI-XML-0001"));
        assert_eq!(
            out.records[0].notional_amount.map(|d| d.to_string()),
            Some("10000000".into())
        );
        assert_eq!(out.records[0].notional_currency.as_deref(), Some("EUR"));
        assert_eq!(
            out.records[0].valuation_amount.map(|d| d.to_string()),
            Some("1500.50".into())
        );
        assert_eq!(out.records[0].valuation_currency.as_deref(), Some("EUR"));
        assert!(out.records[0].reporting_timestamp.is_some());
        // header defaults applied
        assert_eq!(
            out.records[0].entity_responsible_for_reporting.as_deref(),
            Some("DUMMYERR0000000000AA")
        );
        assert_eq!(
            out.records[1].entity_responsible_for_reporting.as_deref(),
            Some("DUMMYERR0000000000AA")
        );
        assert_eq!(out.issues.len(), 0);
    }

    #[test]
    fn unsupported_namespace_flagged_but_still_extracts() {
        let xml = br#"<?xml version="1.0"?>
<EmirReport xmlns="https://example.com/other/v1">
  <Trade>
    <UTI>OPENDQI-XML-NS-1</UTI>
    <Notional currency="EUR">100</Notional>
  </Trade>
</EmirReport>
"#;
        let p = write_tmp("ns.xml", xml);
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();

        assert_eq!(out.records.len(), 1);
        assert_eq!(out.records[0].uti.as_deref(), Some("OPENDQI-XML-NS-1"));
        assert_eq!(out.issues.len(), 1);
        assert_eq!(out.issues[0].check_id, "EMIR.FMT.XML_UNSUPPORTED_NAMESPACE");
        assert_eq!(out.issues[0].severity, Severity::Warning);
    }

    #[test]
    fn unknown_elements_are_deduplicated() {
        let xml = br#"<?xml version="1.0"?>
<EmirReport xmlns="https://opendqi.org/schemas/emir/v0.1">
  <Trade>
    <UTI>X</UTI>
    <FutureField>a</FutureField>
    <FutureField>b</FutureField>
    <OtherFutureField>c</OtherFutureField>
  </Trade>
</EmirReport>
"#;
        let p = write_tmp("unk.xml", xml);
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();

        let unknowns: Vec<&DqIssue> = out
            .issues
            .iter()
            .filter(|i| i.check_id == "EMIR.FMT.XML_UNKNOWN_ELEMENT")
            .collect();
        assert_eq!(unknowns.len(), 2);
        assert_eq!(out.records.len(), 1);
    }

    #[test]
    fn empty_trade_creates_record_with_header_defaults() {
        let xml = br#"<?xml version="1.0"?>
<EmirReport xmlns="https://opendqi.org/schemas/emir/v0.1">
  <Header>
    <EntityResponsibleForReporting>DUMMYERR0000000000AA</EntityResponsibleForReporting>
  </Header>
  <Trade/>
</EmirReport>
"#;
        let p = write_tmp("empty.xml", xml);
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();

        assert_eq!(out.records.len(), 1);
        assert_eq!(out.records[0].uti, None);
        assert_eq!(
            out.records[0].entity_responsible_for_reporting.as_deref(),
            Some("DUMMYERR0000000000AA")
        );
    }
}
