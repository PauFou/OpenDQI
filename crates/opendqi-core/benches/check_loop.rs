//! Benchmarks the check loop (`run_all` / `run_all_sftr`) at 1k, 10k,
//! 100k, and 1M synthetic records (the 1M point is the
//! "millions of records" scale data point; Criterion auto-reduces
//! the sample size for the slow case). Run with:
//!
//! ```text
//! cargo bench -p opendqi-core --bench check_loop
//! ```
//!
//! The synthetic generators produce deterministic records that exercise
//! a mix of action types, asset classes, and date / decimal patterns
//! so most checks evaluate non-trivially. Records are constructed once
//! per group via Criterion's `iter_batched` to keep generation outside
//! the measured region.

use chrono::{NaiveDate, TimeZone, Utc};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use opendqi_core::dq::{default_checks, default_sftr_checks, run_all, run_all_sftr, CheckContext};
use opendqi_core::{EmirRecord, SftrRecord, Thresholds};
use rust_decimal::Decimal;

fn ctx() -> CheckContext {
    let now = Utc.with_ymd_and_hms(2026, 5, 14, 8, 0, 0).unwrap();
    CheckContext {
        thresholds: Thresholds::default(),
        today: now.date_naive(),
        now,
    }
}

fn gen_synthetic_emir(n: usize) -> Vec<EmirRecord> {
    const ACTIONS: &[&str] = &["NEWT", "MODI", "ETRM", "CORR"];
    const ASSET_CLASSES: &[&str] = &["IR", "CR", "FX", "EQ", "CO"];
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let action = ACTIONS[i % ACTIONS.len()];
        let asset_class = ASSET_CLASSES[i % ASSET_CLASSES.len()];
        let notional = Decimal::from((i as i64 + 1) * 1_000_000);
        let day = (i % 365) as u32 + 1;
        out.push(EmirRecord {
            record_id: Some(format!("R{i}")),
            uti: Some(format!("UTI-{i:06}")),
            action_type: Some(action.into()),
            event_type: Some("TRAD".into()),
            asset_class: Some(asset_class.into()),
            counterparty_1: Some("ABCDEFGHIJKLMNOPQR01".into()),
            counterparty_2: Some("ABCDEFGHIJKLMNOPQR02".into()),
            entity_responsible_for_reporting: Some("ABCDEFGHIJKLMNOPQR01".into()),
            notional_amount: Some(notional),
            notional_currency: Some("EUR".into()),
            execution_timestamp: Some(Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap()),
            event_timestamp: Some(Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap()),
            reporting_timestamp: Some(Utc.with_ymd_and_hms(2026, 1, 1, 18, 0, 0).unwrap()),
            effective_date: NaiveDate::from_ymd_opt(2026, 1, day.min(28)),
            maturity_date: NaiveDate::from_ymd_opt(2030, ((i % 12) as u32) + 1, 15),
            valuation_amount: Some(notional / Decimal::from(100)),
            valuation_currency: Some("EUR".into()),
            valuation_timestamp: Some(Utc.with_ymd_and_hms(2026, 5, 13, 8, 0, 0).unwrap()),
            clearing_status: Some(if i % 2 == 0 { "CLRD" } else { "NCLR" }.into()),
            nature: Some(if i % 3 == 0 { "FC" } else { "NFC" }.into()),
            master_agreement_type: Some("ISDA".into()),
            master_agreement_version: Some("2002".into()),
            price: Some(Decimal::new(10_025, 2)),
            price_currency: Some("EUR".into()),
            collateralisation_category: Some("FCOL".into()),
            collateral_portfolio_code: Some(format!("PF-{:04}", i % 50)),
            initial_margin_posted: Some(notional / Decimal::from(20)),
            initial_margin_collected: Some(notional / Decimal::from(20)),
            variation_margin_posted: Some(notional / Decimal::from(50)),
            variation_margin_collected: Some(notional / Decimal::from(50)),
            trading_capacity: Some(if i % 2 == 0 { "PRIN" } else { "AGEN" }.into()),
            intragroup_indicator: Some(i % 7 == 0),
            ..Default::default()
        });
    }
    out
}

fn gen_synthetic_sftr(n: usize) -> Vec<SftrRecord> {
    const ACTIONS: &[&str] = &["NEWT", "MODI", "ETRM", "CORR", "MARU"];
    const SFT_TYPES: &[&str] = &["REPO", "BSB", "SLEB", "MGLD"];
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let action = ACTIONS[i % ACTIONS.len()];
        let sft_type = SFT_TYPES[i % SFT_TYPES.len()];
        let loan = Decimal::from((i as i64 + 1) * 1_000_000);
        out.push(SftrRecord {
            record_id: Some(format!("R{i}")),
            uti: Some(format!("SFT-{i:06}")),
            action_type: Some(action.into()),
            event_type: Some(action.into()),
            sft_type: Some(sft_type.into()),
            counterparty_1: Some("ABCDEFGHIJKLMNOPQR01".into()),
            counterparty_2: Some("ABCDEFGHIJKLMNOPQR02".into()),
            entity_responsible_for_reporting: Some("ABCDEFGHIJKLMNOPQR01".into()),
            loan_value: Some(loan),
            loan_currency: Some("EUR".into()),
            collateral_value: Some(loan + Decimal::from(50_000)),
            collateral_currency: Some("EUR".into()),
            haircut: Some(Decimal::new(5, 2)),
            execution_timestamp: Some(Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap()),
            event_timestamp: Some(Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap()),
            reporting_timestamp: Some(Utc.with_ymd_and_hms(2026, 1, 1, 18, 0, 0).unwrap()),
            effective_date: NaiveDate::from_ymd_opt(2026, 1, 15),
            maturity_date: NaiveDate::from_ymd_opt(2027, 6, 30),
            settlement_date: NaiveDate::from_ymd_opt(2026, 1, 16),
            master_agreement_type: Some("GMRA".into()),
            master_agreement_version: Some("2011".into()),
            collateral_portfolio_code: Some(format!("SFTPF-{:04}", i % 50)),
            reuse_indicator: Some(i % 4 == 0),
            rebate_rate: Some(Decimal::new(15, 4)),
            lending_fee: Some(Decimal::new(25, 4)),
            ..Default::default()
        });
    }
    out
}

fn bench_emir(c: &mut Criterion) {
    let checks = default_checks();
    let ctx = ctx();
    let mut group = c.benchmark_group("run_all_emir");
    for &n in &[1_000usize, 10_000, 100_000, 1_000_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let records = gen_synthetic_emir(n);
            b.iter(|| run_all(&checks, &records, &ctx))
        });
    }
    group.finish();
}

fn bench_sftr(c: &mut Criterion) {
    let checks = default_sftr_checks();
    let ctx = ctx();
    let mut group = c.benchmark_group("run_all_sftr");
    for &n in &[1_000usize, 10_000, 100_000, 1_000_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            let records = gen_synthetic_sftr(n);
            b.iter(|| run_all_sftr(&checks, &records, &ctx))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_emir, bench_sftr);
criterion_main!(benches);
