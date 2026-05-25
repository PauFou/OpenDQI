//! Golden snapshot regression tests for the deterministic outputs
//! (`summary.json` + the per-command issues CSV) across every
//! report-producing command family.
//!
//! Outputs are a stated product guarantee ("deterministic outputs");
//! this harness pins them. It runs the **real built `opendqi`
//! binary** (via `CARGO_BIN_EXE_opendqi`) over the synthetic
//! `examples/` fixtures, normalizes the two non-deterministic axes —
//! absolute paths (workspace root → `<WS>`, temp dir → `<TMP>`) and
//! the `summary.json` wall-clock timestamps (`started_at`/
//! `finished_at` → epoch) — and asserts byte-equality against the
//! committed goldens under `tests/golden/`.
//!
//! `report.html` is intentionally excluded (minijinja + timestamps —
//! not stably snapshotable).
//!
//! Regenerate after an intentional change:
//! ```text
//! UPDATE_GOLDEN=1 cargo test -p opendqi-cli --test golden
//! ```
//! then review the diff before committing. Dependency-free (std only).

use std::path::{Path, PathBuf};
use std::process::Command;

fn ws_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .canonicalize()
        .expect("canonical workspace root")
}

/// Absolute, canonical path to a committed example fixture.
fn ex(rel: &str) -> String {
    ws_root()
        .join(rel)
        .canonicalize()
        .unwrap_or_else(|e| panic!("example {rel}: {e}"))
        .to_string_lossy()
        .into_owned()
}

fn unique_dir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "opendqi-golden-{}-{}-{tag}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Replace the volatile day-count in the margin-state staleness
/// message (`… is <N> days old (threshold <M>).`, where `<N>` =
/// `today − state_as_of` and so grows by one every calendar day) with
/// a stable placeholder. Same determinism intent as `mask_today`;
/// anchored on the fixed `" days old (threshold"` suffix and only the
/// drifting `<N>` is masked (the configured `<M>` threshold is kept).
fn mask_days_old(s: &str) -> String {
    let suffix = " days old (threshold";
    let mut out = String::with_capacity(s.len());
    let mut cursor = 0usize;
    while let Some(rel) = s[cursor..].find(suffix) {
        let j = cursor + rel;
        // Walk left over the run of ASCII digits forming `<N>`.
        let bytes = s.as_bytes();
        let mut num_start = j;
        while num_start > 0 && bytes[num_start - 1].is_ascii_digit() {
            num_start -= 1;
        }
        if num_start < j {
            out.push_str(&s[cursor..num_start]);
            out.push_str("<DAYS>");
        } else {
            // No digits before the suffix — emit verbatim.
            out.push_str(&s[cursor..j]);
        }
        out.push_str(suffix);
        cursor = j + suffix.len();
    }
    out.push_str(&s[cursor..]);
    out
}

/// Zero the RFC3339 string value of `"<key>"<ws>:<ws>"..."` in a JSON
/// blob (tolerates pretty-printer whitespace after the key/colon).
fn zero_json_ts(s: &str, key: &str) -> String {
    let key_tok = format!("\"{key}\"");
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(&key_tok) {
        let after_key = i + key_tok.len();
        let tail = &rest[after_key..];
        // Expect optional ws, ':', optional ws, '"' … '"'.
        let colon = match tail.find(':') {
            Some(c) if tail[..c].trim().is_empty() => c,
            _ => {
                // Not a "key": value occurrence — emit verbatim, continue.
                out.push_str(&rest[..after_key]);
                rest = tail;
                continue;
            }
        };
        let q1 = after_key + colon + 1 + tail[colon + 1..].find('"').expect("opening quote");
        let q2 = q1 + 1 + rest[q1 + 1..].find('"').expect("closing quote");
        out.push_str(&rest[..q1 + 1]);
        out.push_str("1970-01-01T00:00:00Z");
        rest = &rest[q2..];
    }
    out.push_str(rest);
    out
}

fn normalize(content: &str, ws: &str, tmp: &str, is_summary: bool) -> String {
    let mut t = content.replace(ws, "<WS>").replace(tmp, "<TMP>");
    t = mask_today(&t);
    t = mask_days_old(&t);
    if is_summary {
        t = zero_json_ts(&t, "started_at");
        t = zero_json_ts(&t, "finished_at");
    }
    t
}

/// Replace the run-date in `today=YYYY-MM-DD` (emitted by the
/// maturity-in-past checks via `ctx.today`) with a stable placeholder,
/// so goldens stay deterministic across calendar-day boundaries — the
/// same intent as `zero_json_ts` for wall-clock timestamps. Targeted
/// (only the `today=` token) to avoid masking unrelated data dates.
fn mask_today(s: &str) -> String {
    let tok = "today=";
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(tok) {
        let after = i + tok.len();
        let date = &rest[after..];
        let looks_like_date = date.len() >= 10
            && date.as_bytes()[..10].iter().enumerate().all(|(k, &b)| {
                if k == 4 || k == 7 {
                    b == b'-'
                } else {
                    b.is_ascii_digit()
                }
            });
        if looks_like_date {
            out.push_str(&rest[..after]);
            out.push_str("<DATE>");
            rest = &rest[after + 10..];
        } else {
            out.push_str(&rest[..after]);
            rest = &rest[after..];
        }
    }
    out.push_str(rest);
    out
}

fn golden_path(case: &str, ext: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{case}.{ext}"))
}

fn check_or_update(case: &str, ext: &str, actual: &str) {
    let path = golden_path(case, ext);
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing golden {}; run `UPDATE_GOLDEN=1 cargo test -p opendqi-cli --test golden`",
            path.display()
        )
    });
    assert_eq!(
        expected,
        actual,
        "\ngolden mismatch for {case}.{ext} ({})\n\
         if this change is intentional, regenerate with \
         `UPDATE_GOLDEN=1 cargo test -p opendqi-cli --test golden` and review the diff\n",
        path.display()
    );
}

/// Run one command into a fresh out dir, then snapshot the produced
/// `summary.json` and the single `*.csv` issues file.
fn run_case(case: &str, args: &[String]) {
    let bin = env!("CARGO_BIN_EXE_opendqi");
    let out = unique_dir(&format!("{case}-out"));
    let ws = ws_root().to_string_lossy().into_owned();
    let tmp = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    let full: Vec<String> = args
        .iter()
        .cloned()
        .chain(["--out".to_string(), out.to_string_lossy().into_owned()])
        .collect();
    let output = Command::new(bin)
        .args(&full)
        .output()
        .unwrap_or_else(|e| panic!("{case}: spawn failed: {e}"));

    let summary = out.join("summary.json");
    assert!(
        summary.exists(),
        "{case}: no summary.json produced.\nargs: {full:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let summary_txt = std::fs::read_to_string(&summary).unwrap();
    check_or_update(
        case,
        "summary.json",
        &normalize(&summary_txt, &ws, &tmp, true),
    );

    let csvs: Vec<PathBuf> = std::fs::read_dir(&out)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("csv"))
        .collect();
    assert_eq!(
        csvs.len(),
        1,
        "{case}: expected exactly one issues CSV, got {csvs:?}"
    );
    let csv_txt = std::fs::read_to_string(&csvs[0]).unwrap();
    check_or_update(case, "issues.csv", &normalize(&csv_txt, &ws, &tmp, false));

    let _ = std::fs::remove_dir_all(&out);
}

fn a(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

macro_rules! golden_test {
    ($name:ident, $case:literal, $args:expr) => {
        #[test]
        fn $name() {
            run_case($case, &$args);
        }
    };
}

// ---- EMIR ----------------------------------------------------------
golden_test!(
    emir_scan_xml,
    "emir-scan-xml",
    a(&["emir", "scan", &ex("examples/emir/sample.xml")])
);
golden_test!(
    emir_scan_csv,
    "emir-scan-csv",
    a(&[
        "emir",
        "scan",
        &ex("examples/emir/sample.csv"),
        "--mapping",
        &ex("examples/emir/sample_mapping.yml")
    ])
);
golden_test!(
    emir_tr_state,
    "emir-tr-state",
    a(&[
        "emir",
        "tr-state-scan",
        &ex("examples/emir/tr_state/auth107-sample.xml")
    ])
);
golden_test!(
    emir_tr_activity,
    "emir-tr-activity",
    a(&[
        "emir",
        "tr-activity-scan",
        &ex("examples/emir/tr_activity/auth030-sample.xml")
    ])
);
golden_test!(
    emir_recon_stats,
    "emir-recon-stats",
    a(&[
        "emir",
        "recon-stats",
        &ex("examples/emir/recon_stats/auth091-sample.xml")
    ])
);
golden_test!(
    emir_warnings,
    "emir-warnings",
    a(&[
        "emir",
        "warnings",
        &ex("examples/emir/warnings/auth106-sample.xml")
    ])
);
golden_test!(
    emir_mar,
    "emir-mar",
    a(&[
        "emir",
        "mar-scan",
        &ex("examples/emir/mar/auth108-sample.xml")
    ])
);
golden_test!(
    emir_msr,
    "emir-msr",
    a(&[
        "emir",
        "msr-scan",
        &ex("examples/emir/msr/auth109-sample.xml")
    ])
);
golden_test!(
    emir_book_reconcile,
    "emir-book-reconcile",
    a(&[
        "emir",
        "book-reconcile",
        "--book",
        &ex("examples/emir/book_reconcile/book.csv"),
        "--tsr",
        &ex("examples/emir/tr_state/auth107-sample.xml"),
        "--mapping",
        &ex("examples/emir/book_reconcile/book_mapping.yml")
    ])
);
golden_test!(
    emir_tr_audit,
    "emir-tr-audit",
    a(&[
        "emir",
        "tr-audit",
        "--tar",
        &ex("examples/emir/tr_activity/auth030-sample.xml"),
        "--tsr",
        &ex("examples/emir/tr_state/auth107-sample.xml"),
        "--feedback",
        &ex("examples/emir/feedback/auth092-sample.xml")
    ])
);
golden_test!(
    emir_collateral_audit,
    "emir-collateral-audit",
    a(&[
        "emir",
        "collateral-audit",
        "--tsr",
        &ex("examples/emir/collateral_audit/tsr.xml"),
        "--msr",
        &ex("examples/emir/collateral_audit/msr.xml")
    ])
);

// `emir data-quality-pack` writes FOUR artefacts (summary.json +
// issues.csv + indicators.csv + evidence.csv) — the standard
// `run_case` helper only handles one issues CSV per command, so
// the DQI pack gets a dedicated test that snapshots all four.
//
// `--as-of` is pinned so that the stale-valuation indicator's
// cutoff is stable across calendar days (same intent as the
// `mask_today` masking we apply to other goldens).
#[test]
fn emir_data_quality_pack() {
    let bin = env!("CARGO_BIN_EXE_opendqi");
    let out = unique_dir("emir-data-quality-pack-out");
    let ws = ws_root().to_string_lossy().into_owned();
    let tmp = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    let args: Vec<String> = a(&[
        "emir",
        "data-quality-pack",
        "--tsr",
        &ex("examples/quickstart-emir/auth107-tsr.xml"),
        "--tar",
        &ex("examples/quickstart-emir/auth030-tar.xml"),
        "--feedback",
        &ex("examples/quickstart-emir/auth092-feedback.xml"),
        // v0.18 E7: add auth.090 EMIR Position Set layer —
        // exercises 4 Position DQIs + 4 EMIR.POS.* granular checks.
        "--positions",
        &ex("examples/emir/positions/auth090-sample.xml"),
        // v0.18 F1: add auth.091 Reconciliation Statistics —
        // dual-purpose parse activates the 7 cross-CP DQIs that
        // self-reported `not_applicable` since v0.16 B1.
        "--recon-stats",
        &ex("examples/emir/recon_stats/auth091-sample.xml"),
        "--as-of",
        "2026-05-21",
    ])
    .into_iter()
    .chain(["--out".to_string(), out.to_string_lossy().into_owned()])
    .collect();

    let output = Command::new(bin)
        .args(&args)
        .output()
        .expect("spawn opendqi");

    assert!(
        out.join("summary.json").exists(),
        "no summary.json produced.\nargs: {args:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    for (rel, ext, is_summary) in [
        ("summary.json", "summary.json", true),
        ("issues.csv", "issues.csv", false),
        ("indicators.csv", "indicators.csv", false),
        ("evidence.csv", "evidence.csv", false),
    ] {
        let path = out.join(rel);
        assert!(path.exists(), "missing {rel} in DQI pack output");
        let txt = std::fs::read_to_string(&path).unwrap();
        check_or_update(
            "emir-data-quality-pack",
            ext,
            &normalize(&txt, &ws, &tmp, is_summary),
        );
    }

    let _ = std::fs::remove_dir_all(&out);
}

// ---- EMIR (store-backed) -------------------------------------------
#[test]
fn emir_feedback() {
    let store = unique_dir("emir-feedback-store").join("h.db");
    run_case(
        "emir-feedback",
        &a(&[
            "emir",
            "feedback",
            &ex("examples/emir/feedback/auth092-sample.xml"),
            "--store",
            &store.to_string_lossy(),
        ]),
    );
}
// (No `emir reconcile`: EMIR has no reconciliation message — removed
// in Milestone 0.6. `EMIR.REC.*` is exercised via `emir recon-stats`
// over the real auth.091 per-transaction detail, covered by
// `recon_stats_integration.rs` + the `emir-recon-stats` golden.)

// ---- SFTR ----------------------------------------------------------
golden_test!(
    sftr_scan,
    "sftr-scan",
    a(&["sftr", "scan", &ex("examples/sftr/iso20022/sample.xml")])
);
golden_test!(
    sftr_tr_state,
    "sftr-tr-state",
    a(&[
        "sftr",
        "tr-state-scan",
        &ex("examples/sftr/tr_state/auth079-sample.xml")
    ])
);
golden_test!(
    sftr_tr_activity,
    "sftr-tr-activity",
    a(&[
        "sftr",
        "tr-activity-scan",
        &ex("examples/sftr/tr_activity/auth052-tar-sample.xml")
    ])
);
golden_test!(
    sftr_missing_collateral,
    "sftr-missing-collateral",
    a(&[
        "sftr",
        "missing-collateral",
        &ex("examples/sftr/missing_collateral/auth083-sample.xml")
    ])
);
golden_test!(
    sftr_tr_audit,
    "sftr-tr-audit",
    a(&[
        "sftr",
        "tr-audit",
        "--tar",
        &ex("examples/sftr/tr_activity/auth052-tar-sample.xml"),
        "--tsr",
        &ex("examples/sftr/tr_state/auth079-sample.xml")
    ])
);
golden_test!(
    sftr_book_reconcile,
    "sftr-book-reconcile",
    a(&[
        "sftr",
        "book-reconcile",
        "--book",
        &ex("examples/sftr/book_reconcile/book.csv"),
        "--tsr",
        &ex("examples/sftr/tr_state/auth079-sample.xml"),
        "--mapping",
        &ex("examples/sftr/book_reconcile/book_mapping.yml")
    ])
);
// Dedicated test for `opendqi sftr data-quality-pack` (mirror of
// `emir_data_quality_pack` above). Snapshots all four artefacts
// because the standard `run_case` helper only handles one issues
// CSV per command.
//
// `--as-of` is pinned so that the SFTR stale-loan-value indicator
// cutoff is stable across calendar days.
#[test]
fn sftr_data_quality_pack() {
    let bin = env!("CARGO_BIN_EXE_opendqi");
    let out = unique_dir("sftr-data-quality-pack-out");
    let ws = ws_root().to_string_lossy().into_owned();
    let tmp = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    let args: Vec<String> = a(&[
        "sftr",
        "data-quality-pack",
        "--tsr",
        &ex("examples/sftr/tr_state/auth079-sample.xml"),
        "--tar",
        &ex("examples/sftr/tr_activity/auth052-tar-sample.xml"),
        "--as-of",
        "2026-05-21",
    ])
    .into_iter()
    .chain(["--out".to_string(), out.to_string_lossy().into_owned()])
    .collect();

    let output = Command::new(bin)
        .args(&args)
        .output()
        .expect("spawn opendqi");

    assert!(
        out.join("summary.json").exists(),
        "no summary.json produced.\nargs: {args:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    for (rel, ext, is_summary) in [
        ("summary.json", "summary.json", true),
        ("issues.csv", "issues.csv", false),
        ("indicators.csv", "indicators.csv", false),
        ("evidence.csv", "evidence.csv", false),
    ] {
        let path = out.join(rel);
        assert!(path.exists(), "missing {rel} in SFTR DQI pack output");
        let txt = std::fs::read_to_string(&path).unwrap();
        check_or_update(
            "sftr-data-quality-pack",
            ext,
            &normalize(&txt, &ws, &tmp, is_summary),
        );
    }

    let _ = std::fs::remove_dir_all(&out);
}

// v0.17 G1': 5-layer variant of the SFTR DQI pack golden that
// exercises EVERY indicator path (TSR + TAR + auth.080
// reconciliation + auth.083 missing-collateral + auth.085 MSR).
// Snapshots all four artefacts. The MSR layer triggers the new
// SFTR.T3.* granular checks from F1' so the issues stream is
// observably distinct from the 2-layer golden.
#[test]
fn sftr_data_quality_pack_full() {
    let bin = env!("CARGO_BIN_EXE_opendqi");
    let out = unique_dir("sftr-data-quality-pack-full-out");
    let ws = ws_root().to_string_lossy().into_owned();
    let tmp = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    let args: Vec<String> = a(&[
        "sftr",
        "data-quality-pack",
        "--tsr",
        &ex("examples/sftr/tr_state/auth079-sample.xml"),
        "--tar",
        &ex("examples/sftr/tr_activity/auth052-tar-sample.xml"),
        "--reconciliation",
        &ex("examples/sftr/reconciliation/auth080-sample.xml"),
        "--missing-collateral",
        &ex("examples/sftr/missing_collateral/auth083-sample.xml"),
        "--msr",
        &ex("examples/sftr/margin_state/auth085-sample.xml"),
        // v0.18 A6: add auth.070 MAR layer to the canonical
        // full-coverage golden — exercises 3 MAR DQIs + 4
        // SFTR.MAR.* granular checks on the same path.
        "--mar",
        &ex("examples/sftr/margin_activity/auth070-sample.xml"),
        // v0.18 B5: add auth.071 reuse activity layer —
        // exercises 2 reuse DQIs + 2 SFTR.REU.* granular checks.
        "--reuse-activity",
        &ex("examples/sftr/reuse_activity/auth071-sample.xml"),
        // v0.18 C5: add auth.086 reuse state layer —
        // exercises 2 reuse-state DQIs + 2 SFTR.REU.STATE.*
        // granular checks.
        "--reuse-state",
        &ex("examples/sftr/reuse_state/auth086-sample.xml"),
        // v0.18 D3: add auth.084 transaction status advice —
        // exercises DQI_REJ_RATE_SFTR.
        "--tr-status-advice",
        &ex("examples/sftr/tr_status_advice/auth084-sample.xml"),
        "--as-of",
        "2026-05-21",
    ])
    .into_iter()
    .chain(["--out".to_string(), out.to_string_lossy().into_owned()])
    .collect();

    let output = Command::new(bin)
        .args(&args)
        .output()
        .expect("spawn opendqi");

    assert!(
        out.join("summary.json").exists(),
        "no summary.json produced.\nargs: {args:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    for (rel, ext, is_summary) in [
        ("summary.json", "summary.json", true),
        ("issues.csv", "issues.csv", false),
        ("indicators.csv", "indicators.csv", false),
        ("evidence.csv", "evidence.csv", false),
    ] {
        let path = out.join(rel);
        assert!(path.exists(), "missing {rel} in SFTR DQI pack full output");
        let txt = std::fs::read_to_string(&path).unwrap();
        check_or_update(
            "sftr-data-quality-pack-full",
            ext,
            &normalize(&txt, &ws, &tmp, is_summary),
        );
    }

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn sftr_reconcile() {
    let store = unique_dir("sftr-reconcile-store").join("h.db");
    run_case(
        "sftr-reconcile",
        &a(&[
            "sftr",
            "reconcile",
            &ex("examples/sftr/reconciliation/auth080-sample.xml"),
            "--store",
            &store.to_string_lossy(),
        ]),
    );
}
