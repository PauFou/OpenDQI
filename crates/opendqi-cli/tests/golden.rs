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
    if is_summary {
        t = zero_json_ts(&t, "started_at");
        t = zero_json_ts(&t, "finished_at");
    }
    t
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
#[test]
fn emir_reconcile() {
    let store = unique_dir("emir-reconcile-store").join("h.db");
    run_case(
        "emir-reconcile",
        &a(&[
            "emir",
            "reconcile",
            &ex("examples/emir/reconciliation/auth106-sample.xml"),
            "--store",
            &store.to_string_lossy(),
        ]),
    );
}

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
#[test]
fn sftr_reconcile() {
    let store = unique_dir("sftr-reconcile-store").join("h.db");
    run_case(
        "sftr-reconcile",
        &a(&[
            "sftr",
            "reconcile",
            &ex("examples/sftr/reconciliation/auth083-sample.xml"),
            "--store",
            &store.to_string_lossy(),
        ]),
    );
}
