//! Extract `EmirRecord` values from the official ISO 20022
//! `auth.030.001.03` DerivativesTradeReport format used by ESMA under
//! EMIR Refit.
//!
//! Strategy: streaming `quick_xml::NsReader`, no full schema model.
//! A stack tracks the current element path; each leaf is committed
//! either through a typed match table or stashed in
//! [`EmirRecord::raw_fields`] (catch-all).
//!
//! ## Legal note
//!
//! The auth.030 XSD itself is SWIFT-licensed and is NOT redistributed
//! with OpenDQI. This adapter implements interoperability with a
//! public regulatory standard (Regulation (EU) 2022/1855); the
//! element names and structure used here are facts of that protocol.

use std::path::Path;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use opendqi_core::EmirRecord;
use quick_xml::events::{BytesStart, Event};
use quick_xml::NsReader;
use rust_decimal::Decimal;
use tracing::warn;

use super::XmlReadOutcome;

const ACTION_PARENT: &str = "Rpt";

/// Read an EMIR auth.030.001.03 file. The caller is expected to have
/// already validated well-formedness (done by `route::read_emir_xml`).
pub fn read(path: &Path) -> anyhow::Result<XmlReadOutcome> {
    let source_label = path.to_string_lossy().into_owned();
    let mut reader = NsReader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut pile: Vec<String> = Vec::new();
    let mut is_leaf: Vec<bool> = Vec::new();
    let mut text_buf = String::new();
    let mut attrs_buf: Vec<(String, String)> = Vec::new();

    let mut current: Option<EmirRecord> = None;
    let mut action_depth: Option<usize> = None;
    let mut records: Vec<EmirRecord> = Vec::new();
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

    Ok(XmlReadOutcome {
        records,
        issues: Vec::new(),
    })
}

fn start_record(source_label: &str, rpt_index: u32, action_code: &'static str) -> EmirRecord {
    EmirRecord {
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
        "TradEx" => "OTHR",
        _ => return None,
    })
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

fn commit_leaf(rec: &mut EmirRecord, rel_path: &str, text: &str, attrs: &[(String, String)]) {
    let trimmed = text.trim();
    let record_id = rec.record_id.clone().unwrap_or_default();

    // Typed routing table. Order matters where alternatives exist
    // (e.g. counterparty_2 from Org/LEI takes precedence over a bare
    // LEI). The match is verbose on purpose — easy to grep.
    match rel_path {
        // Counterparty / entity
        "CtrPtySpcfcData/NttyRspnsblForRpt/LEI" => {
            set_str(&mut rec.entity_responsible_for_reporting, trimmed)
        }
        "CtrPtySpcfcData/RptgCtrPty/Id/LEI" => {
            set_str(&mut rec.counterparty_1, trimmed);
            // fallback for ERR if not set yet
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
        "CtrPtySpcfcData/RptgCtrPty/TradgCpcty" => set_str(&mut rec.trading_capacity, trimmed),
        "CtrPtySpcfcData/RptgCtrPty/Ntr" => set_str(&mut rec.nature, trimmed),
        "CtrPtySpcfcData/RptgCtrPty/CorpSctr/Fnncl"
        | "CtrPtySpcfcData/RptgCtrPty/CorpSctr/NonFnncl" => {
            set_str(&mut rec.corporate_sector, trimmed)
        }
        "CtrPtySpcfcData/RptgCtrPty/RptgOblgtn" => {
            set_str(&mut rec.reporting_obligation_indicator, trimmed)
        }
        "CtrPtySpcfcData/RptgCtrPty/CmrclOrTrsrFincg" => {
            set_bool(&mut rec.commercial_or_treasury_financing, trimmed)
        }

        // Contract data
        "CmonTradData/CtrctData/AsstClss" => set_str(&mut rec.asset_class, trimmed),
        "CmonTradData/CtrctData/PdctClssfctn" => set_str(&mut rec.product_id, trimmed),
        "CmonTradData/CtrctData/UndrlygInstrm/Sngl/Indx/Id" => {
            set_str(&mut rec.underlying_id, trimmed)
        }
        "CmonTradData/CtrctData/UndrlygInstrm/Sngl/ISIN" => {
            if rec.underlying_id.is_none() {
                set_str(&mut rec.underlying_id, trimmed);
            }
        }
        "CmonTradData/CtrctData/MstrAgrmt/Tp" => set_str(&mut rec.master_agreement_type, trimmed),
        "CmonTradData/CtrctData/MstrAgrmt/Vrsn" => {
            set_str(&mut rec.master_agreement_version, trimmed)
        }

        // Trade identifiers
        "CmonTradData/TxData/TradId/UnqTxIdr" => set_str(&mut rec.uti, trimmed),
        "CmonTradData/TxData/TradId/PrrUnqTxIdr/UnqTxIdr" => set_str(&mut rec.prior_uti, trimmed),

        // Event metadata
        "CmonTradData/TxData/EvtTp" => set_str(&mut rec.event_type, trimmed),
        "CmonTradData/TxData/ExctnTmStmp" => set_dt(
            &mut rec.execution_timestamp,
            trimmed,
            "ExctnTmStmp",
            &record_id,
        ),
        "CmonTradData/TxData/EvtTmStmp" => {
            set_dt(&mut rec.event_timestamp, trimmed, "EvtTmStmp", &record_id)
        }
        "CmonTradData/TxData/RptgTmStmp" => set_dt(
            &mut rec.reporting_timestamp,
            trimmed,
            "RptgTmStmp",
            &record_id,
        ),
        "CmonTradData/TxData/EffctvDt" => {
            set_date(&mut rec.effective_date, trimmed, "EffctvDt", &record_id)
        }
        "CmonTradData/TxData/MtrtyDt" => {
            set_date(&mut rec.maturity_date, trimmed, "MtrtyDt", &record_id)
        }
        "CmonTradData/TxData/TermntnDt" => {
            set_date(&mut rec.termination_date, trimmed, "TermntnDt", &record_id)
        }

        // Confirmation method
        "CmonTradData/CtrctData/Cnfrmtn/Tp" | "CmonTradData/TxData/Cnfrmtn/Tp" => {
            set_str(&mut rec.confirmation_method, trimmed)
        }

        // Notional, both legs
        "CmonTradData/TxData/NtnlAmt/Lgs/FrstLgNtnl/Amt" | "CmonTradData/TxData/NtnlAmt/Amt" => {
            set_decimal(&mut rec.notional_amount, trimmed, "Notional", &record_id);
            set_str(
                &mut rec.notional_currency,
                attr_of(attrs, "Ccy").unwrap_or("").trim(),
            );
        }
        "CmonTradData/TxData/NtnlAmt/Lgs/ScndLgNtnl/Amt" => {
            set_decimal(
                &mut rec.leg2_notional_amount,
                trimmed,
                "Leg2Notional",
                &record_id,
            );
            set_str(
                &mut rec.leg2_notional_currency,
                attr_of(attrs, "Ccy").unwrap_or("").trim(),
            );
        }
        "CmonTradData/TxData/NtnlAmt/Lgs/FrstLgNtnl/PmtFrqcy" => {
            set_str(&mut rec.leg1_payment_frequency, trimmed)
        }
        "CmonTradData/TxData/NtnlAmt/Lgs/ScndLgNtnl/PmtFrqcy" => {
            set_str(&mut rec.leg2_payment_frequency, trimmed)
        }

        // Price
        "CmonTradData/TxData/Pric/Amt" | "CmonTradData/TxData/Pric/Pric/Amt" => {
            set_decimal(&mut rec.price, trimmed, "Pric", &record_id);
            set_str(
                &mut rec.price_currency,
                attr_of(attrs, "Ccy").unwrap_or("").trim(),
            );
        }

        // Valuation
        "CmonTradData/Valtn/Amt" => {
            set_decimal(&mut rec.valuation_amount, trimmed, "Valtn", &record_id);
            set_str(
                &mut rec.valuation_currency,
                attr_of(attrs, "Ccy").unwrap_or("").trim(),
            );
        }
        "CmonTradData/Valtn/TmStmp" => set_dt(
            &mut rec.valuation_timestamp,
            trimmed,
            "ValtnTmStmp",
            &record_id,
        ),
        "CmonTradData/Valtn/Tp" => set_str(&mut rec.valuation_type, trimmed),
        "CmonTradData/Valtn/Dlt" => set_decimal(&mut rec.delta, trimmed, "Delta", &record_id),
        "CmonTradData/Valtn/Gmma" => set_decimal(&mut rec.gamma, trimmed, "Gamma", &record_id),
        "CmonTradData/Valtn/Vega" => set_decimal(&mut rec.vega, trimmed, "Vega", &record_id),
        "CmonTradData/Valtn/Chng" | "CmonTradData/Valtn/MtmChng" => {
            set_decimal(&mut rec.mtm_value_change, trimmed, "MtmChng", &record_id)
        }

        // Collateral
        "CmonTradData/Coll/PrtflCd" => set_str(&mut rec.collateral_portfolio_code, trimmed),
        "CmonTradData/Coll/CollstnInd" => set_str(&mut rec.collateralisation_category, trimmed),

        // Clearing
        "CmonTradData/Clr/Clrd" => {
            let normalized = match trimmed.to_ascii_lowercase().as_str() {
                "true" | "1" => Some("CLRD".to_owned()),
                "false" | "0" => Some("NCLR".to_owned()),
                "" => None,
                _ => Some(trimmed.to_owned()),
            };
            if let Some(v) = normalized {
                rec.clearing_status = Some(v);
            }
        }
        "CmonTradData/Clr/CCP/LEI" => set_str(&mut rec.clearing_ccp_lei, trimmed),

        // Indicators
        "CmonTradData/NtraGrpTx" | "CmonTradData/CtrctData/NtraGrpTx" => {
            set_bool(&mut rec.intragroup_indicator, trimmed)
        }
        "CmonTradData/CtrctData/DrctlyLnkdToCmrclActvtyOrTrsr"
        | "CmonTradData/DrctlyLnkdToCmrclActvtyOrTrsr" => {
            set_bool(&mut rec.hedging_indicator, trimmed)
        }

        // Margin (under MrgnUpd action, paths are typically shorter)
        "InitlMrgnPstd/Amt" | "InitlMrgnPstd/PstUcclrdInitlMrgn/Amt" => set_decimal(
            &mut rec.initial_margin_posted,
            trimmed,
            "InitlMrgnPstd",
            &record_id,
        ),
        "InitlMrgnColl/Amt" | "InitlMrgnColl/PstUcclrdInitlMrgn/Amt" => set_decimal(
            &mut rec.initial_margin_collected,
            trimmed,
            "InitlMrgnColl",
            &record_id,
        ),
        "VartnMrgnPstd/Amt" | "VartnMrgnPstd/PstUcclrdVartnMrgn/Amt" => set_decimal(
            &mut rec.variation_margin_posted,
            trimmed,
            "VartnMrgnPstd",
            &record_id,
        ),
        "VartnMrgnColl/Amt" | "VartnMrgnColl/PstUcclrdVartnMrgn/Amt" => set_decimal(
            &mut rec.variation_margin_collected,
            trimmed,
            "VartnMrgnColl",
            &record_id,
        ),

        _ => {
            // Catch-all: preserve everything we did not explicitly map.
            // Skip purely empty leaves with no attribute payload.
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

fn set_decimal(dst: &mut Option<Decimal>, s: &str, field: &str, record_id: &str) {
    if s.is_empty() {
        return;
    }
    match Decimal::from_str(s) {
        Ok(d) => *dst = Some(d),
        Err(e) => {
            warn!(record = %record_id, field, value = s, error = %e, "could not parse decimal")
        }
    }
}

fn set_date(dst: &mut Option<NaiveDate>, s: &str, field: &str, record_id: &str) {
    if s.is_empty() {
        return;
    }
    match NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        Ok(d) => *dst = Some(d),
        Err(e) => warn!(record = %record_id, field, value = s, error = %e, "could not parse date"),
    }
}

fn set_dt(dst: &mut Option<DateTime<Utc>>, s: &str, field: &str, record_id: &str) {
    if s.is_empty() {
        return;
    }
    match DateTime::parse_from_rfc3339(s) {
        Ok(d) => *dst = Some(d.with_timezone(&Utc)),
        Err(e) => {
            warn!(record = %record_id, field, value = s, error = %e, "could not parse datetime")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, content: &[u8]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("opendqi-iso-{}-{name}", std::process::id()));
        std::fs::write(&p, content).unwrap();
        p
    }

    fn wrap(body: &str) -> Vec<u8> {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:auth.030.001.03">
  <DerivsTradRpt>
    <TradData>
      <Rpt>{body}</Rpt>
    </TradData>
  </DerivsTradRpt>
</Document>"#
        )
        .into_bytes()
    }

    #[test]
    fn extracts_uti_from_new_report() {
        let p = write_tmp(
            "new.xml",
            &wrap(
                r#"<New><CmonTradData><TxData><TradId><UnqTxIdr>OPENDQI-ISO-0001</UnqTxIdr></TradId></TxData></CmonTradData></New>"#,
            ),
        );
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(out.records.len(), 1);
        assert_eq!(out.records[0].uti.as_deref(), Some("OPENDQI-ISO-0001"));
        assert_eq!(out.records[0].action_type.as_deref(), Some("NEWT"));
    }

    #[test]
    fn derives_action_type_from_envelope() {
        let body = r#"
<New><CmonTradData><TxData><TradId><UnqTxIdr>A1</UnqTxIdr></TradId></TxData></CmonTradData></New>
</Rpt><Rpt>
<Mod><CmonTradData><TxData><TradId><UnqTxIdr>A2</UnqTxIdr></TradId></TxData></CmonTradData></Mod>
</Rpt><Rpt>
<Crrctn><CmonTradData><TxData><TradId><UnqTxIdr>A3</UnqTxIdr></TradId></TxData></CmonTradData></Crrctn>
</Rpt><Rpt>
<EarlyTermntn><CmonTradData><TxData><TradId><UnqTxIdr>A4</UnqTxIdr></TradId></TxData></CmonTradData></EarlyTermntn>
</Rpt><Rpt>
<ValtnUpd><CmonTradData><TxData><TradId><UnqTxIdr>A5</UnqTxIdr></TradId></TxData></CmonTradData></ValtnUpd>
</Rpt><Rpt>
<PosCmpnt><CmonTradData><TxData><TradId><UnqTxIdr>A6</UnqTxIdr></TradId></TxData></CmonTradData></PosCmpnt>
</Rpt><Rpt>
<MrgnUpd><CmonTradData><TxData><TradId><UnqTxIdr>A7</UnqTxIdr></TradId></TxData></CmonTradData></MrgnUpd>
</Rpt><Rpt>
<TradEx><CmonTradData><TxData><TradId><UnqTxIdr>A8</UnqTxIdr></TradId></TxData></CmonTradData></TradEx>
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
            vec!["NEWT", "MODI", "CORR", "ETRM", "VALU", "POSC", "MARU", "OTHR"]
        );
    }

    #[test]
    fn notional_currency_attribute_captured() {
        let body = r#"<New><CmonTradData><TxData><TradId><UnqTxIdr>X</UnqTxIdr></TradId><NtnlAmt><Lgs><FrstLgNtnl><Amt Ccy="EUR">10000000</Amt></FrstLgNtnl></Lgs></NtnlAmt></TxData></CmonTradData></New>"#;
        let p = write_tmp("notional.xml", &wrap(body));
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        let r = &out.records[0];
        assert_eq!(
            r.notional_amount.map(|d| d.to_string()),
            Some("10000000".into())
        );
        assert_eq!(r.notional_currency.as_deref(), Some("EUR"));
    }

    #[test]
    fn header_lei_falls_back_to_reporting_counterparty() {
        let body = r#"<New><CtrPtySpcfcData><RptgCtrPty><Id><LEI>DUMMYCPTY100000000AA</LEI></Id></RptgCtrPty></CtrPtySpcfcData><CmonTradData><TxData><TradId><UnqTxIdr>X</UnqTxIdr></TradId></TxData></CmonTradData></New>"#;
        let p = write_tmp("fallback.xml", &wrap(body));
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        let r = &out.records[0];
        assert_eq!(r.counterparty_1.as_deref(), Some("DUMMYCPTY100000000AA"));
        assert_eq!(
            r.entity_responsible_for_reporting.as_deref(),
            Some("DUMMYCPTY100000000AA")
        );
    }

    #[test]
    fn multiple_reports_in_one_file() {
        let body = r#"
<New><CmonTradData><TxData><TradId><UnqTxIdr>A</UnqTxIdr></TradId></TxData></CmonTradData></New>
</Rpt><Rpt>
<New><CmonTradData><TxData><TradId><UnqTxIdr>B</UnqTxIdr></TradId></TxData></CmonTradData></New>
</Rpt><Rpt>
<New><CmonTradData><TxData><TradId><UnqTxIdr>C</UnqTxIdr></TradId></TxData></CmonTradData></New>
"#;
        let p = write_tmp("multi.xml", &wrap(body));
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(out.records.len(), 3);
        let utis: Vec<&str> = out
            .records
            .iter()
            .filter_map(|r| r.uti.as_deref())
            .collect();
        assert_eq!(utis, vec!["A", "B", "C"]);
    }

    #[test]
    fn swap_two_legs_captured() {
        let body = r#"<New><CmonTradData><TxData><TradId><UnqTxIdr>X</UnqTxIdr></TradId><NtnlAmt><Lgs><FrstLgNtnl><Amt Ccy="EUR">1000</Amt></FrstLgNtnl><ScndLgNtnl><Amt Ccy="USD">1100</Amt></ScndLgNtnl></Lgs></NtnlAmt></TxData></CmonTradData></New>"#;
        let p = write_tmp("swap.xml", &wrap(body));
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        let r = &out.records[0];
        assert_eq!(
            r.notional_amount.map(|d| d.to_string()),
            Some("1000".into())
        );
        assert_eq!(r.notional_currency.as_deref(), Some("EUR"));
        assert_eq!(
            r.leg2_notional_amount.map(|d| d.to_string()),
            Some("1100".into())
        );
        assert_eq!(r.leg2_notional_currency.as_deref(), Some("USD"));
    }

    #[test]
    fn margin_update_fills_four_margin_fields() {
        // MrgnUpd children are direct, not via CmonTradData.
        let body = r#"<MrgnUpd><InitlMrgnPstd><Amt Ccy="EUR">100</Amt></InitlMrgnPstd><InitlMrgnColl><Amt Ccy="EUR">200</Amt></InitlMrgnColl><VartnMrgnPstd><Amt Ccy="EUR">10</Amt></VartnMrgnPstd><VartnMrgnColl><Amt Ccy="EUR">20</Amt></VartnMrgnColl></MrgnUpd>"#;
        let p = write_tmp("margin.xml", &wrap(body));
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        let r = &out.records[0];
        assert_eq!(r.action_type.as_deref(), Some("MARU"));
        assert_eq!(
            r.initial_margin_posted.map(|d| d.to_string()),
            Some("100".into())
        );
        assert_eq!(
            r.initial_margin_collected.map(|d| d.to_string()),
            Some("200".into())
        );
        assert_eq!(
            r.variation_margin_posted.map(|d| d.to_string()),
            Some("10".into())
        );
        assert_eq!(
            r.variation_margin_collected.map(|d| d.to_string()),
            Some("20".into())
        );
    }

    #[test]
    fn clearing_ccp_lei_and_status_captured() {
        let body = r#"<New><CmonTradData><TxData><TradId><UnqTxIdr>X</UnqTxIdr></TradId></TxData><Clr><Clrd>true</Clrd><CCP><LEI>LCHLDNUS00000000A</LEI></CCP></Clr></CmonTradData></New>"#;
        let p = write_tmp("clearing.xml", &wrap(body));
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        let r = &out.records[0];
        assert_eq!(r.clearing_status.as_deref(), Some("CLRD"));
        assert_eq!(r.clearing_ccp_lei.as_deref(), Some("LCHLDNUS00000000A"));
    }

    #[test]
    fn unmapped_leaf_lands_in_raw_fields() {
        let body = r#"<New><CmonTradData><TxData><TradId><UnqTxIdr>X</UnqTxIdr></TradId><FuturisticField>treasure</FuturisticField></TxData></CmonTradData></New>"#;
        let p = write_tmp("raw.xml", &wrap(body));
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        let r = &out.records[0];
        assert_eq!(
            r.raw_fields
                .get("CmonTradData/TxData/FuturisticField")
                .map(String::as_str),
            Some("treasure")
        );
    }

    #[test]
    fn prior_uti_captured_on_modification() {
        let body = r#"<Mod><CmonTradData><TxData><TradId><UnqTxIdr>NEW</UnqTxIdr><PrrUnqTxIdr><UnqTxIdr>OLD</UnqTxIdr></PrrUnqTxIdr></TradId></TxData></CmonTradData></Mod>"#;
        let p = write_tmp("prior.xml", &wrap(body));
        let out = read(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        let r = &out.records[0];
        assert_eq!(r.uti.as_deref(), Some("NEW"));
        assert_eq!(r.prior_uti.as_deref(), Some("OLD"));
        assert_eq!(r.action_type.as_deref(), Some("MODI"));
    }
}
