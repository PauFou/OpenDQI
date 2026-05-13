//! Extract `SftrRecord` values from the official ISO 20022
//! `auth.052.001.02` SecuritiesFinancingReportingTransactionReport
//! format used by ESMA.
//!
//! Strategy: streaming `quick_xml::NsReader`, selective routing
//! table, `raw_fields` catch-all — same approach as the EMIR
//! `auth.030` extractor in `crates/opendqi-xml/src/emir/iso20022.rs`.
//!
//! ## Legal note
//!
//! The auth.052 XSD itself is SWIFT-licensed and is NOT redistributed
//! with OpenDQI.

use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::SftrRecord;
use quick_xml::events::{BytesStart, Event};
use quick_xml::NsReader;
use rust_decimal::Decimal;
use tracing::warn;

use super::SftrXmlReadOutcome;

const ACTION_PARENT: &str = "Rpt";

/// Read an SFTR auth.052.001.02 file. The caller is expected to have
/// already validated well-formedness (done by `sftr/route.rs`).
pub fn read(path: &Path) -> anyhow::Result<SftrXmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();
    let mut attrs_buf: Vec<(String, String)> = Vec::new();

    let mut current: Option<SftrRecord> = None;
    let mut action_depth: Option<usize> = None;
    let mut records: Vec<SftrRecord> = Vec::new();
    let mut rpt_index: u32 = 0;

    loop {
        match reader.read_resolved_event_into(&mut buf)? {
            (_, Event::Start(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local.clone());
                text_buf.clear();
                attrs_buf = collect_attrs(&e);

                if current.is_none() {
                    if let Some(action) = detect_action_start(&pile) {
                        rpt_index += 1;
                        current = Some(start_record(&source_label, rpt_index, action));
                        action_depth = Some(pile.len());
                    }
                } else if let Some(rec) = current.as_mut() {
                    // We may be entering an SFT-type wrapper just under LnData.
                    detect_sft_type(&pile, action_depth, rec);
                }
            }
            (_, Event::Empty(e)) => {
                let local = local_name(&e);
                push_element(&mut pile, &mut is_leaf, local.clone());
                let attrs = collect_attrs(&e);

                if current.is_none() {
                    if let Some(action) = detect_action_start(&pile) {
                        rpt_index += 1;
                        records.push(start_record(&source_label, rpt_index, action));
                        pop_element(&mut pile, &mut is_leaf);
                        continue;
                    }
                }

                if let (Some(rec), Some(adepth)) = (current.as_mut(), action_depth) {
                    if pile.len() > adepth {
                        let rel = pile[adepth..].join("/");
                        commit_leaf(rec, &rel, "", &attrs);
                    }
                }
                pop_element(&mut pile, &mut is_leaf);
                attrs_buf.clear();
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
                if let (Some(rec), Some(adepth)) = (current.as_mut(), action_depth) {
                    if leaf_now && pile.len() > adepth {
                        let rel = pile[adepth..].join("/");
                        commit_leaf(rec, &rel, &text_buf, &attrs_buf);
                    }
                }
                if let Some(adepth) = action_depth {
                    if pile.len() == adepth {
                        if let Some(rec) = current.take() {
                            records.push(rec);
                        }
                        action_depth = None;
                    }
                }
                pop_element(&mut pile, &mut is_leaf);
                text_buf.clear();
                attrs_buf.clear();
            }
            (_, Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(SftrXmlReadOutcome {
        records,
        issues: Vec::new(),
    })
}

fn start_record(source_label: &str, rpt_index: u32, action_code: &'static str) -> SftrRecord {
    SftrRecord {
        source_file: Some(source_label.to_owned()),
        record_id: Some(format!("{source_label}#rpt-{rpt_index}")),
        action_type: Some(action_code.to_owned()),
        ..Default::default()
    }
}

fn detect_action_start(pile: &[String]) -> Option<&'static str> {
    if pile.len() < 2 || pile[pile.len() - 2] != ACTION_PARENT {
        return None;
    }
    action_code(&pile[pile.len() - 1])
}

fn action_code(local: &str) -> Option<&'static str> {
    Some(match local {
        "New" => "NEWT",
        "Mod" => "MODI",
        "Crrctn" => "CORR",
        "EarlyTermntn" => "ETRM",
        "PosCmpnt" => "POSC",
        "ValtnUpd" => "VALU",
        "MrgnUpd" => "MARU",
        "CollUpd" => "COLU",
        "ReuseUpd" => "REUU",
        "TradEx" => "OTHR",
        _ => return None,
    })
}

/// When we enter a child of `LnData`, record the SFT type on the
/// current record (Repo→REPO, BsbSctyTrad→BSB, etc.).
fn detect_sft_type(pile: &[String], action_depth: Option<usize>, rec: &mut SftrRecord) {
    let adepth = match action_depth {
        Some(d) => d,
        None => return,
    };
    if pile.len() != adepth + 2 {
        return;
    }
    if pile[adepth] != "LnData" {
        return;
    }
    let code = match pile[adepth + 1].as_str() {
        "Repo" => "REPO",
        "BsbSctyTrad" => "BSB",
        "SctyLndg" => "SLEB",
        "MgnLndg" => "MGLD",
        _ => return,
    };
    rec.sft_type = Some(code.to_owned());
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

fn collect_attrs(e: &BytesStart<'_>) -> Vec<(String, String)> {
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

fn attr_of<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn commit_leaf(rec: &mut SftrRecord, rel_path: &str, text: &str, attrs: &[(String, String)]) {
    let trimmed = text.trim();
    let record_id = rec.record_id.clone().unwrap_or_default();

    // The match table lists all 4 SFT-type wrappers explicitly. Verbose
    // but trivially debuggable.
    match rel_path {
        // Counterparty / entity
        "CtrPtySpcfcData/NttyRspnsblForRpt/LEI" => {
            set_str(&mut rec.entity_responsible_for_reporting, trimmed)
        }
        "CtrPtySpcfcData/RptgCtrPty/Id/LEI" => {
            set_str(&mut rec.counterparty_1, trimmed);
            if rec.entity_responsible_for_reporting.is_none() {
                set_str(&mut rec.entity_responsible_for_reporting, trimmed);
            }
        }
        "CtrPtySpcfcData/OthrCtrPty/Id/LEI" | "CtrPtySpcfcData/OthrCtrPty/Id/Org/LEI" => {
            set_str(&mut rec.counterparty_2, trimmed)
        }
        "CtrPtySpcfcData/OthrCtrPty/Id/Org/AnyBIC" => {
            if rec.counterparty_2.is_none() {
                set_str(&mut rec.counterparty_2, trimmed);
            }
        }

        // UTI / prior UTI — under any of the 4 SFT-type wrappers.
        "LnData/Repo/UnqTxIdr"
        | "LnData/BsbSctyTrad/UnqTxIdr"
        | "LnData/SctyLndg/UnqTxIdr"
        | "LnData/MgnLndg/UnqTxIdr" => set_str(&mut rec.uti, trimmed),
        "LnData/Repo/PrrUnqTxIdr/UnqTxIdr"
        | "LnData/BsbSctyTrad/PrrUnqTxIdr/UnqTxIdr"
        | "LnData/SctyLndg/PrrUnqTxIdr/UnqTxIdr"
        | "LnData/MgnLndg/PrrUnqTxIdr/UnqTxIdr" => set_str(&mut rec.prior_uti, trimmed),

        // Event metadata
        "LnData/Repo/EvtTp"
        | "LnData/BsbSctyTrad/EvtTp"
        | "LnData/SctyLndg/EvtTp"
        | "LnData/MgnLndg/EvtTp" => set_str(&mut rec.event_type, trimmed),
        "LnData/Repo/ExctnDtTm"
        | "LnData/BsbSctyTrad/ExctnDtTm"
        | "LnData/SctyLndg/ExctnDtTm"
        | "LnData/MgnLndg/ExctnDtTm" => {
            set_dt(
                &mut rec.execution_timestamp,
                trimmed,
                "ExctnDtTm",
                &record_id,
            );
            // event_timestamp defaults to execution if not provided separately
            if rec.event_timestamp.is_none() {
                rec.event_timestamp = rec.execution_timestamp;
            }
        }
        "LnData/Repo/EvtDtTm"
        | "LnData/BsbSctyTrad/EvtDtTm"
        | "LnData/SctyLndg/EvtDtTm"
        | "LnData/MgnLndg/EvtDtTm" => {
            set_dt(&mut rec.event_timestamp, trimmed, "EvtDtTm", &record_id)
        }
        "LnData/Repo/RptgDtTm"
        | "LnData/BsbSctyTrad/RptgDtTm"
        | "LnData/SctyLndg/RptgDtTm"
        | "LnData/MgnLndg/RptgDtTm" => set_dt(
            &mut rec.reporting_timestamp,
            trimmed,
            "RptgDtTm",
            &record_id,
        ),

        // Dates
        "LnData/Repo/EvntDt"
        | "LnData/BsbSctyTrad/EvntDt"
        | "LnData/SctyLndg/EvntDt"
        | "LnData/MgnLndg/EvntDt" => {
            set_date(&mut rec.effective_date, trimmed, "EvntDt", &record_id)
        }
        "LnData/Repo/MtrtyDt"
        | "LnData/BsbSctyTrad/MtrtyDt"
        | "LnData/SctyLndg/MtrtyDt"
        | "LnData/MgnLndg/MtrtyDt" => {
            set_date(&mut rec.maturity_date, trimmed, "MtrtyDt", &record_id)
        }
        "LnData/Repo/TermntnDt"
        | "LnData/BsbSctyTrad/TermntnDt"
        | "LnData/SctyLndg/TermntnDt"
        | "LnData/MgnLndg/TermntnDt" => {
            set_date(&mut rec.termination_date, trimmed, "TermntnDt", &record_id)
        }
        "LnData/Repo/SttlmDt"
        | "LnData/BsbSctyTrad/SttlmDt"
        | "LnData/SctyLndg/SttlmDt"
        | "LnData/MgnLndg/SttlmDt" => {
            set_date(&mut rec.settlement_date, trimmed, "SttlmDt", &record_id)
        }

        // Loan / principal
        "LnData/Repo/PrncplAmt/Amt"
        | "LnData/BsbSctyTrad/PrncplAmt/Amt"
        | "LnData/SctyLndg/PrncplAmt/Amt"
        | "LnData/MgnLndg/PrncplAmt/Amt"
        | "LnData/Repo/LnVal/Amt"
        | "LnData/BsbSctyTrad/LnVal/Amt"
        | "LnData/SctyLndg/LnVal/Amt"
        | "LnData/MgnLndg/LnVal/Amt" => {
            set_decimal(&mut rec.loan_value, trimmed, "LoanValue", &record_id);
            set_str(
                &mut rec.loan_currency,
                attr_of(attrs, "Ccy").unwrap_or("").trim(),
            );
        }

        // Master agreement
        "LnData/Repo/MstrAgrmt/Tp"
        | "LnData/BsbSctyTrad/MstrAgrmt/Tp"
        | "LnData/SctyLndg/MstrAgrmt/Tp"
        | "LnData/MgnLndg/MstrAgrmt/Tp" => set_str(&mut rec.master_agreement_type, trimmed),
        "LnData/Repo/MstrAgrmt/Vrsn"
        | "LnData/BsbSctyTrad/MstrAgrmt/Vrsn"
        | "LnData/SctyLndg/MstrAgrmt/Vrsn"
        | "LnData/MgnLndg/MstrAgrmt/Vrsn" => set_str(&mut rec.master_agreement_version, trimmed),

        // SFT-specific rates
        "LnData/Repo/RbtRate" => set_decimal(&mut rec.rebate_rate, trimmed, "RbtRate", &record_id),
        "LnData/SctyLndg/LndgFee" => {
            set_decimal(&mut rec.lending_fee, trimmed, "LndgFee", &record_id)
        }

        // Collateral
        "CollData/CollVal/Amt" | "CollData/Amt" => {
            set_decimal(&mut rec.collateral_value, trimmed, "CollVal", &record_id);
            set_str(
                &mut rec.collateral_currency,
                attr_of(attrs, "Ccy").unwrap_or("").trim(),
            );
        }
        "CollData/Hrcut" => set_decimal(&mut rec.haircut, trimmed, "Hrcut", &record_id),
        "CollData/AvlblForCollReuse" => set_bool(&mut rec.reuse_indicator, trimmed),
        "CollData/PrtflCd" => set_str(&mut rec.collateral_portfolio_code, trimmed),
        "CollData/Sctys/Id" | "CollData/Sctys/ISIN" | "CollData/ISIN" => {
            set_str(&mut rec.collateral_isin, trimmed)
        }

        _ => {
            if trimmed.is_empty() && attrs.is_empty() {
                return;
            }
            let encoded = encode_value(trimmed, attrs);
            rec.raw_fields.insert(rel_path.to_owned(), encoded);
        }
    }
}

fn encode_value(text: &str, attrs: &[(String, String)]) -> String {
    if attrs.is_empty() {
        return text.to_owned();
    }
    let mut out = String::from(text);
    for (k, v) in attrs {
        out.push('|');
        out.push_str(k);
        out.push('=');
        out.push_str(v);
    }
    out
}

fn set_str(dst: &mut Option<String>, s: &str) {
    if !s.is_empty() {
        *dst = Some(s.to_owned());
    }
}

fn set_bool(dst: &mut Option<bool>, s: &str) {
    match s.to_ascii_lowercase().as_str() {
        "true" | "1" | "y" | "yes" => *dst = Some(true),
        "false" | "0" | "n" | "no" => *dst = Some(false),
        _ => {}
    }
}

fn set_decimal(dst: &mut Option<Decimal>, s: &str, field: &str, record: &str) {
    if s.is_empty() {
        return;
    }
    match Decimal::from_str(s) {
        Ok(d) => *dst = Some(d),
        Err(e) => warn!(record = %record, field, value = s, error = %e, "could not parse decimal"),
    }
}

fn set_date(dst: &mut Option<NaiveDate>, s: &str, field: &str, record: &str) {
    if s.is_empty() {
        return;
    }
    match NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        Ok(d) => *dst = Some(d),
        Err(e) => warn!(record = %record, field, value = s, error = %e, "could not parse date"),
    }
}

fn set_dt(dst: &mut Option<DateTime<Utc>>, s: &str, field: &str, record: &str) {
    if s.is_empty() {
        return;
    }
    match DateTime::parse_from_rfc3339(s) {
        Ok(d) => *dst = Some(d.with_timezone(&Utc)),
        Err(e) => warn!(record = %record, field, value = s, error = %e, "could not parse datetime"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, content: &[u8]) -> std::path::PathBuf {
        let p =
            std::env::temp_dir().join(format!("opendqi-sftr-iso-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    fn wrap(body: &str) -> Vec<u8> {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.052.001.02">
  <SctiesFincgRptgTxRpt>
    <TradData>
      <Rpt>{body}</Rpt>
    </TradData>
  </SctiesFincgRptgTxRpt>
</Document>"#
        )
        .into_bytes()
    }

    #[test]
    fn extracts_uti_from_new_repo() {
        let p = write_tmp(
            "new_repo.xml",
            &wrap(r#"<New><LnData><Repo><UnqTxIdr>SFTR-ISO-001</UnqTxIdr></Repo></LnData></New>"#),
        );
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(out.records.len(), 1);
        assert_eq!(out.records[0].uti.as_deref(), Some("SFTR-ISO-001"));
        assert_eq!(out.records[0].action_type.as_deref(), Some("NEWT"));
        assert_eq!(out.records[0].sft_type.as_deref(), Some("REPO"));
    }

    #[test]
    fn sft_type_inferred_from_wrapper() {
        let body = r#"
<New><LnData><Repo><UnqTxIdr>R1</UnqTxIdr></Repo></LnData></New>
</Rpt><Rpt>
<New><LnData><BsbSctyTrad><UnqTxIdr>B1</UnqTxIdr></BsbSctyTrad></LnData></New>
</Rpt><Rpt>
<New><LnData><SctyLndg><UnqTxIdr>S1</UnqTxIdr></SctyLndg></LnData></New>
</Rpt><Rpt>
<New><LnData><MgnLndg><UnqTxIdr>M1</UnqTxIdr></MgnLndg></LnData></New>
"#;
        let p = write_tmp("sft_types.xml", &wrap(body));
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        let types: Vec<&str> = out
            .records
            .iter()
            .filter_map(|r| r.sft_type.as_deref())
            .collect();
        assert_eq!(types, vec!["REPO", "BSB", "SLEB", "MGLD"]);
    }

    #[test]
    fn loan_principal_with_currency() {
        let body = r#"<New><LnData><Repo><UnqTxIdr>X</UnqTxIdr><PrncplAmt><Amt Ccy="EUR">5000000</Amt></PrncplAmt></Repo></LnData></New>"#;
        let p = write_tmp("loan.xml", &wrap(body));
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        let r = &out.records[0];
        assert_eq!(r.loan_value.map(|d| d.to_string()), Some("5000000".into()));
        assert_eq!(r.loan_currency.as_deref(), Some("EUR"));
    }

    #[test]
    fn collateral_data_captured() {
        let body = r#"<New><LnData><Repo><UnqTxIdr>X</UnqTxIdr></Repo></LnData><CollData><CollVal><Amt Ccy="EUR">100000</Amt></CollVal><Hrcut>0.05</Hrcut><AvlblForCollReuse>true</AvlblForCollReuse></CollData></New>"#;
        let p = write_tmp("coll.xml", &wrap(body));
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        let r = &out.records[0];
        assert_eq!(
            r.collateral_value.map(|d| d.to_string()),
            Some("100000".into())
        );
        assert_eq!(r.collateral_currency.as_deref(), Some("EUR"));
        assert_eq!(r.haircut.map(|d| d.to_string()), Some("0.05".into()));
        assert_eq!(r.reuse_indicator, Some(true));
    }

    #[test]
    fn action_types_recognised() {
        let body = r#"
<New><LnData><Repo><UnqTxIdr>1</UnqTxIdr></Repo></LnData></New>
</Rpt><Rpt>
<Mod><LnData><Repo><UnqTxIdr>2</UnqTxIdr></Repo></LnData></Mod>
</Rpt><Rpt>
<Crrctn><LnData><Repo><UnqTxIdr>3</UnqTxIdr></Repo></LnData></Crrctn>
</Rpt><Rpt>
<EarlyTermntn><LnData><Repo><UnqTxIdr>4</UnqTxIdr></Repo></LnData></EarlyTermntn>
</Rpt><Rpt>
<ValtnUpd><LnData><Repo><UnqTxIdr>5</UnqTxIdr></Repo></LnData></ValtnUpd>
</Rpt><Rpt>
<CollUpd><LnData><Repo><UnqTxIdr>6</UnqTxIdr></Repo></LnData></CollUpd>
</Rpt><Rpt>
<MrgnUpd><LnData><Repo><UnqTxIdr>7</UnqTxIdr></Repo></LnData></MrgnUpd>
</Rpt><Rpt>
<ReuseUpd><LnData><Repo><UnqTxIdr>8</UnqTxIdr></Repo></LnData></ReuseUpd>
"#;
        let p = write_tmp("actions.xml", &wrap(body));
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        let codes: Vec<&str> = out
            .records
            .iter()
            .filter_map(|r| r.action_type.as_deref())
            .collect();
        assert_eq!(
            codes,
            vec!["NEWT", "MODI", "CORR", "ETRM", "VALU", "COLU", "MARU", "REUU"]
        );
    }

    #[test]
    fn raw_fields_captures_unmapped_leaf() {
        let body = r#"<New><LnData><Repo><UnqTxIdr>X</UnqTxIdr><FutureField>treasure</FutureField></Repo></LnData></New>"#;
        let p = write_tmp("raw.xml", &wrap(body));
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        let r = &out.records[0];
        assert_eq!(
            r.raw_fields
                .get("LnData/Repo/FutureField")
                .map(String::as_str),
            Some("treasure")
        );
    }
}
