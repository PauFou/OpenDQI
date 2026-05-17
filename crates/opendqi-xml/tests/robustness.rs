//! Parser robustness contract (see `docs/reliability.md`).
//!
//! Every public `opendqi-xml` entry point must, for **any** byte
//! input — empty, malformed, truncated, invalid-UTF-8, hostile — and
//! for deterministically-mutated valid fixtures, return `Ok(outcome)`
//! or a clean `Err`, and **never panic / hang**. A panic or a breach
//! of the wall-clock bound is a defect (fix at the parser chokepoint
//! with a bounded guard).
//!
//! Dependency-free (std only). Deterministic: the mutation loop uses a
//! fixed-seed LCG so failures are reproducible.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use opendqi_xml::{
    check_wellformedness, read_emir_feedback_xml, read_emir_mar_xml, read_emir_msr_xml,
    read_emir_recon_stats_xml, read_emir_tr_state_xml, read_emir_warnings_xml, read_emir_xml,
    read_sftr_missing_collateral_xml, read_sftr_reconciliation_xml, read_sftr_tr_state_xml,
    read_sftr_xml,
};

/// A named parser entry point adapted to `&Path -> ()` (return value
/// dropped — we only assert "does not panic / hang").
type Ep = (&'static str, fn(&Path));

/// All entry points.
fn entry_points() -> Vec<Ep> {
    vec![
        ("read_emir_xml", |p| {
            let _ = read_emir_xml(p);
        }),
        ("read_sftr_xml", |p| {
            let _ = read_sftr_xml(p);
        }),
        ("read_emir_feedback_xml", |p| {
            let _ = read_emir_feedback_xml(p);
        }),
        ("read_emir_tr_state_xml", |p| {
            let _ = read_emir_tr_state_xml(p);
        }),
        ("read_sftr_tr_state_xml", |p| {
            let _ = read_sftr_tr_state_xml(p);
        }),
        ("read_emir_recon_stats_xml", |p| {
            let _ = read_emir_recon_stats_xml(p);
        }),
        ("read_emir_warnings_xml", |p| {
            let _ = read_emir_warnings_xml(p);
        }),
        ("read_sftr_reconciliation_xml", |p| {
            let _ = read_sftr_reconciliation_xml(p);
        }),
        ("read_sftr_missing_collateral_xml", |p| {
            let _ = read_sftr_missing_collateral_xml(p);
        }),
        ("read_emir_mar_xml", |p| {
            let _ = read_emir_mar_xml(p);
        }),
        ("read_emir_msr_xml", |p| {
            let _ = read_emir_msr_xml(p);
        }),
        ("check_wellformedness", |p| {
            let _ = check_wellformedness(p);
        }),
    ]
}

fn quiet_panics() {
    // This file is its own test binary; a no-op hook keeps the log
    // readable when a *deliberately* caught panic occurs. Idempotent.
    std::panic::set_hook(Box::new(|_| {}));
}

fn write_tmp(bytes: &[u8]) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "opendqi-robust-{}-{}.xml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&p, bytes).unwrap();
    p
}

/// Fast path: call `f(path)` directly, catching panics. No thread /
/// no timeout — for inputs that cannot plausibly hang (small / lightly
/// mutated). A panic fails the test with a precise label.
fn no_panic(label: &str, bytes: &[u8], f: fn(&Path)) {
    let path = write_tmp(bytes);
    let p = path.clone();
    let r = catch_unwind(AssertUnwindSafe(|| f(&p)));
    let _ = std::fs::remove_file(&path);
    assert!(
        r.is_ok(),
        "PANIC: `{label}` panicked on hostile input ({} bytes)",
        bytes.len()
    );
}

/// Bounded path: run `f(path)` in a worker thread with a wall-clock
/// timeout — for the size/depth "bomb" inputs that could blow up. A
/// panic *or* a timeout fails the test with a precise label.
fn bounded(label: &str, bytes: &[u8], f: fn(&Path)) {
    let path = write_tmp(bytes);
    let p2 = path.clone();
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let r = catch_unwind(AssertUnwindSafe(|| f(&p2)));
        let _ = tx.send(r.is_ok());
    });
    let verdict = rx.recv_timeout(Duration::from_secs(15));
    let _ = std::fs::remove_file(&path);
    match verdict {
        Ok(true) => {}
        Ok(false) => panic!(
            "PANIC: `{label}` panicked on hostile input ({} bytes)",
            bytes.len()
        ),
        Err(_) => panic!(
            "HANG: `{label}` exceeded the 15s bound on hostile input ({} bytes) — \
             DoS-shaped blowup; add a bounded guard at the parser chokepoint",
            bytes.len()
        ),
    }
    // On a hang the worker is detached (std cannot cancel a thread);
    // the test has already failed, the process exits at suite end.
    let _ = worker.join();
}

/// Fixed, hand-authored hostile corpus.
fn hostile_corpus() -> Vec<(&'static str, Vec<u8>)> {
    let big = 2 * 1024 * 1024; // 2 MiB
    vec![
        ("empty", b"".to_vec()),
        ("whitespace", b"   \n\t  \r\n".to_vec()),
        ("not-xml", b"this is not xml at all \x00\x01\x02".to_vec()),
        ("nul-bytes", b"<Document>\x00\x00\x00</Document>".to_vec()),
        (
            "invalid-utf8",
            vec![0xff, 0xfe, 0x00, 0x80, 0xc0, 0xaf, 0x3c, 0x41],
        ),
        (
            "xml-decl-only",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>".to_vec(),
        ),
        ("truncated-tag", b"<Document><Tx><UnqIdr".to_vec()),
        ("unterminated-attr", b"<Document attr=\"oops".to_vec()),
        ("unclosed-tree", b"<a><b><c><d>text".to_vec()),
        ("mismatched-tags", b"<a></b><c></d>".to_vec()),
        (
            "wrong-namespace",
            b"<Document xmlns=\"urn:totally:bogus\"><X/></Document>".to_vec(),
        ),
        (
            "no-namespace",
            b"<Document><Tx><UnqIdr>U1</UnqIdr></Tx></Document>".to_vec(),
        ),
        (
            "huge-attr",
            [
                b"<Document a=\"".to_vec(),
                vec![b'x'; big],
                b"\"/>".to_vec(),
            ]
            .concat(),
        ),
        (
            "huge-text",
            [
                b"<Document>".to_vec(),
                vec![b'x'; big],
                b"</Document>".to_vec(),
            ]
            .concat(),
        ),
        ("deep-nesting", {
            let d = 20_000;
            let mut v = Vec::with_capacity(d * 8);
            v.extend_from_slice(b"<Document>");
            for _ in 0..d {
                v.extend_from_slice(b"<a>");
            }
            for _ in 0..d {
                v.extend_from_slice(b"</a>");
            }
            v.extend_from_slice(b"</Document>");
            v
        }),
        ("many-siblings", {
            let mut v = Vec::with_capacity(50_000 * 4 + 32);
            v.extend_from_slice(b"<Document>");
            for _ in 0..50_000 {
                v.extend_from_slice(b"<X/>");
            }
            v.extend_from_slice(b"</Document>");
            v
        }),
        (
            "billion-laughs",
            br#"<?xml version="1.0"?>
<!DOCTYPE lolz [
 <!ENTITY a "aaaaaaaaaa">
 <!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;">
 <!ENTITY c "&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;">
 <!ENTITY d "&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;">
 <!ENTITY e "&d;&d;&d;&d;&d;&d;&d;&d;&d;&d;">
]>
<Document>&e;</Document>"#
                .to_vec(),
        ),
        (
            "valid-prefix-then-garbage",
            [
                b"<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:auth.030.001.03\"><".to_vec(),
                vec![0u8; 4096],
            ]
            .concat(),
        ),
    ]
}

/// Size/depth "bomb" cases get the time-bounded path; the rest run
/// on the fast path.
fn is_bomb(cname: &str) -> bool {
    matches!(
        cname,
        "huge-attr"
            | "huge-text"
            | "deep-nesting"
            | "many-siblings"
            | "billion-laughs"
            | "valid-prefix-then-garbage"
    )
}

#[test]
fn every_entry_point_survives_the_hostile_corpus() {
    quiet_panics();
    for (cname, bytes) in hostile_corpus() {
        for (ename, f) in entry_points() {
            let label = format!("{ename} / corpus:{cname}");
            if is_bomb(cname) {
                bounded(&label, &bytes, f);
            } else {
                no_panic(&label, &bytes, f);
            }
        }
    }
}

/// Proves the harness actually catches a panic (guards against false
/// confidence): a parser that panics MUST fail the test.
#[test]
#[should_panic(expected = "PANIC:")]
fn harness_detects_a_panicking_parser() {
    quiet_panics();
    no_panic("selftest", b"anything", |_p| {
        panic!("injected parser panic")
    });
}

/// Tiny deterministic LCG (Numerical Recipes constants) — no `rand`.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}

fn valid_fixtures() -> Vec<PathBuf> {
    let ws = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    [
        "examples/emir/sample.xml",
        "examples/emir/tr_state/auth107-sample.xml",
        "examples/emir/feedback/auth092-sample.xml",
        "examples/emir/recon_stats/auth091-sample.xml",
        "examples/emir/warnings/auth106-sample.xml",
        "examples/emir/mar/auth108-sample.xml",
        "examples/sftr/iso20022/sample.xml",
        "examples/sftr/tr_state/auth079-sample.xml",
        "examples/sftr/reconciliation/auth080-sample.xml",
        "examples/sftr/missing_collateral/auth083-sample.xml",
    ]
    .iter()
    .map(|r| ws.join(r))
    .collect()
}

#[test]
fn deterministic_mutation_of_valid_fixtures_never_panics() {
    quiet_panics();
    let mut rng = Lcg(0x0DD1_5EED_C0FF_EE42);
    let eps = entry_points();
    for fx in valid_fixtures() {
        let base = std::fs::read(&fx).unwrap();
        for _ in 0..120 {
            let mut m = base.clone();
            if m.is_empty() {
                continue;
            }
            // 1–4 mutations per variant.
            for _ in 0..(1 + (rng.next() % 4)) {
                match rng.next() % 4 {
                    0 => {
                        // bit flip
                        let i = (rng.next() as usize) % m.len();
                        m[i] ^= 1 << (rng.next() % 8);
                    }
                    1 => {
                        // byte set
                        let i = (rng.next() as usize) % m.len();
                        m[i] = (rng.next() & 0xff) as u8;
                    }
                    2 => {
                        // truncate
                        let i = (rng.next() as usize) % m.len();
                        m.truncate(i);
                    }
                    _ => {
                        // insert
                        let i = (rng.next() as usize) % (m.len() + 1);
                        m.insert(i, (rng.next() & 0xff) as u8);
                    }
                }
            }
            for (ename, f) in &eps {
                no_panic(
                    &format!(
                        "{ename} / mutated:{}",
                        fx.file_name().unwrap().to_string_lossy()
                    ),
                    &m,
                    *f,
                );
            }
        }
    }
}
