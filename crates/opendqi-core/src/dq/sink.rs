//! Streaming sorted-issue engine (Milestone 0.22 — Increment B of
//! the no-retain issue pipeline).
//!
//! [`SortedIssueSink`] buffers issues, applying severity overrides
//! and online [`IssueAggregator`] tallying as they arrive. Once the
//! in-memory buffer reaches `max_in_memory`, the buffer is sorted by
//! [`super::issue_cmp`] and spilled to a temp **JSON-Lines** run
//! file. [`SortedIssueSink::finish`] yields the `ScanSummary` plus a
//! [`SortedIssues`] iterator that produces every issue in the exact
//! `issue_cmp` order:
//!
//! - **no-spill path** (buffer never exceeded the threshold — always
//!   true for the small golden fixtures): one in-memory
//!   `sort_unstable_by(issue_cmp)`, i.e. *literally* the
//!   `finalize_issues` result ⇒ byte-identical;
//! - **spill path**: a k-way merge of the sorted run files using the
//!   same `issue_cmp` ⇒ same multiset, non-decreasing under
//!   `issue_cmp` (among `issue_cmp`-equal issues the order is
//!   unspecified — exactly as the already-`sort_unstable` finalize).
//!
//! Dormant in B (unwired); Increment C flips `run_scan` /
//! `write_issues_csv` onto it. Equivalence to `finalize_issues` is
//! locked by the tests below. The spill is transient (same
//! process/version, written→read→deleted within one scan); a
//! [`SpillDir`] RAII guard removes it on drop (full consumption,
//! early drop, or error).

use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

use chrono::{DateTime, Utc};

use crate::config::Thresholds;
use crate::model::{DqIssue, Regime, ScanSummary, Severity};

use super::{issue_cmp, IssueAggregator};

/// RAII guard: best-effort removal of the spill temp dir (and every
/// run file under it) when dropped.
#[derive(Debug)]
struct SpillDir {
    dir: PathBuf,
}

impl Drop for SpillDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Buffers issues; spills sorted JSON-Lines runs at `max_in_memory`.
pub struct SortedIssueSink {
    buffer: Vec<DqIssue>,
    max_in_memory: usize,
    aggregator: IssueAggregator,
    overrides: BTreeMap<String, Severity>,
    runs: Vec<PathBuf>,
    spill_dir: Option<SpillDir>,
}

impl SortedIssueSink {
    /// `max_in_memory` is the buffer length that triggers a spill —
    /// parameterised so tests can force tiny multi-run cases; the
    /// production default is chosen/measured in Increment C. Clamped
    /// to ≥ 1.
    pub fn new(thresholds: &Thresholds, max_in_memory: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_in_memory: max_in_memory.max(1),
            aggregator: IssueAggregator::new(),
            overrides: thresholds.severity_overrides.clone(),
            runs: Vec::new(),
            spill_dir: None,
        }
    }

    /// Apply the severity override (same lookup as
    /// `apply_severity_overrides`; no-op on the common empty map),
    /// tally it online, buffer it, and spill if the buffer is full.
    pub fn push(&mut self, mut issue: DqIssue) {
        if let Some(sev) = self.overrides.get(&issue.check_id) {
            issue.severity = *sev;
        }
        self.aggregator.observe(&issue);
        self.buffer.push(issue);
        if self.buffer.len() >= self.max_in_memory {
            self.spill();
        }
    }

    fn ensure_spill_dir(&mut self) -> PathBuf {
        if self.spill_dir.is_none() {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let dir = std::env::temp_dir().join(format!(
                "opendqi-spill-{}-{}",
                std::process::id(),
                nanos
            ));
            let _ = fs::create_dir_all(&dir);
            self.spill_dir = Some(SpillDir { dir });
        }
        self.spill_dir
            .as_ref()
            .expect("spill dir just set")
            .dir
            .clone()
    }

    /// Sort the buffer by `issue_cmp` and write it as one JSONL run.
    /// On any I/O / serialisation failure the (sorted) buffer is
    /// **kept in memory** rather than lose or misorder issues — that
    /// degrades the memory bound, never correctness; `finish` then
    /// merges the residual from memory.
    fn spill(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        self.buffer.sort_unstable_by(issue_cmp);
        let idx = self.runs.len();
        let dir = self.ensure_spill_dir();
        let path = dir.join(format!("run-{idx:06}.jsonl"));
        let Ok(file) = File::create(&path) else {
            return;
        };
        let mut w = BufWriter::new(file);
        for issue in &self.buffer {
            let Ok(line) = serde_json::to_string(issue) else {
                return; // keep buffer; residual merge handles it
            };
            if writeln!(w, "{line}").is_err() {
                return;
            }
        }
        if w.flush().is_ok() {
            self.runs.push(path);
            self.buffer.clear();
        }
    }

    /// Produce the summary and a sorted iterator over every issue.
    pub fn finish(
        mut self,
        regime: Regime,
        files_processed: u32,
        records_processed: u32,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
    ) -> (ScanSummary, SortedIssues) {
        let summary = std::mem::take(&mut self.aggregator).into_summary(
            regime,
            files_processed,
            records_processed,
            started_at,
            finished_at,
        );

        if self.runs.is_empty() {
            // No-spill: a single in-memory sort — *literally* the
            // `finalize_issues` result (override already applied per
            // push) ⇒ byte-identical.
            let mut buf = std::mem::take(&mut self.buffer);
            buf.sort_unstable_by(issue_cmp);
            return (summary, SortedIssues::InMemory(buf.into_iter()));
        }

        // Spill path: flush the residual buffer as the last run so
        // the merge is uniform over run files.
        self.spill();
        let runs = std::mem::take(&mut self.runs);
        let spill = self
            .spill_dir
            .take()
            .expect("spill dir set whenever runs is non-empty");

        let mut readers: Vec<BufReader<File>> = Vec::with_capacity(runs.len());
        let mut heap: BinaryHeap<HeapItem> = BinaryHeap::with_capacity(runs.len());
        for path in &runs {
            // A run we just wrote being unreadable is essentially
            // impossible in-process; skip it (never truncate / panic)
            // — `run` indexes `readers` so the remaining indices stay
            // contiguous and aligned.
            let Ok(file) = File::open(path) else {
                continue;
            };
            let mut reader = BufReader::new(file);
            let run = readers.len();
            if let Some(issue) = read_issue(&mut reader) {
                heap.push(HeapItem { issue, run });
            }
            readers.push(reader);
        }
        (
            summary,
            SortedIssues::Merge(MergeIter {
                readers,
                heap,
                _spill: spill,
            }),
        )
    }
}

/// Read one JSONL `DqIssue` from `reader`; `None` at EOF or on a
/// blank / unparenable line (defensive — the run was written by this
/// same process/version).
fn read_issue(reader: &mut BufReader<File>) -> Option<DqIssue> {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return None,
            Ok(_) => {
                let trimmed = line.trim_end_matches(['\n', '\r']);
                if trimmed.is_empty() {
                    continue;
                }
                return serde_json::from_str::<DqIssue>(trimmed).ok();
            }
            Err(_) => return None,
        }
    }
}

/// One spilled-run head, ordered for a **min-by-`issue_cmp`**
/// `BinaryHeap` (which is a max-heap, so the comparison is reversed).
struct HeapItem {
    issue: DqIssue,
    run: usize,
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.run == other.run && issue_cmp(&self.issue, &other.issue) == Ordering::Equal
    }
}
impl Eq for HeapItem {}
impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reversed so the `issue_cmp`-smallest is the heap maximum;
        // run index (also reversed) only makes ties deterministic.
        issue_cmp(&other.issue, &self.issue).then_with(|| other.run.cmp(&self.run))
    }
}
impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// k-way merge over the sorted run files.
pub struct MergeIter {
    readers: Vec<BufReader<File>>,
    heap: BinaryHeap<HeapItem>,
    _spill: SpillDir, // RAII: temp dir removed when the iter drops
}

/// Issues in the exact [`super::issue_cmp`] order. Consumed by the
/// (Increment C) streaming `issues.csv` writer.
pub enum SortedIssues {
    /// No-spill path: a single in-memory `issue_cmp` sort —
    /// byte-identical to `finalize_issues`.
    InMemory(std::vec::IntoIter<DqIssue>),
    /// Spill path: a k-way merge of the sorted run files.
    Merge(MergeIter),
}

impl Iterator for SortedIssues {
    type Item = DqIssue;
    fn next(&mut self) -> Option<DqIssue> {
        match self {
            SortedIssues::InMemory(it) => it.next(),
            SortedIssues::Merge(m) => {
                let HeapItem { issue, run } = m.heap.pop()?;
                if let Some(next) = read_issue(&mut m.readers[run]) {
                    m.heap.push(HeapItem { issue: next, run });
                }
                Some(issue)
            }
        }
    }
}

#[cfg(test)]
impl SortedIssues {
    /// Spill dir path while the merge iterator is alive (`None` for
    /// the in-memory path) — test-only RAII assertion hook.
    pub(crate) fn spill_path(&self) -> Option<PathBuf> {
        match self {
            SortedIssues::Merge(m) => Some(m._spill.dir.clone()),
            SortedIssues::InMemory(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dq::{finalize_issues, CheckContext};
    use crate::model::DqDimension;

    fn ctx(t: &Thresholds) -> CheckContext {
        CheckContext {
            thresholds: t.clone(),
            today: chrono::DateTime::from_timestamp(0, 0).unwrap().date_naive(),
            now: chrono::DateTime::from_timestamp(0, 0).unwrap(),
        }
    }
    fn ts() -> DateTime<Utc> {
        DateTime::from_timestamp(0, 0).unwrap()
    }

    fn mk(check: &str, sev: Severity, msg: &str) -> DqIssue {
        DqIssue {
            check_id: check.into(),
            regime: Regime::Emir,
            severity: sev,
            dimension: DqDimension::Completeness,
            record_id: None,
            uti: None,
            field: None,
            value: None,
            message: msg.into(),
            source_file: None,
            evidence: Vec::new(),
        }
    }

    fn ser(issues: &[DqIssue]) -> Vec<String> {
        issues
            .iter()
            .map(|i| serde_json::to_string(i).unwrap())
            .collect()
    }

    /// Core property: the sink (any threshold) is finalize-equivalent
    /// — same length, non-decreasing under `issue_cmp`, identical
    /// multiset; and the **no-spill** path is byte-equal to
    /// `finalize_issues`.
    fn assert_equiv(xs: &[DqIssue], thr: &Thresholds, threshold: usize) {
        let mut expected = xs.to_vec();
        finalize_issues(&mut expected, &ctx(thr));

        let mut sink = SortedIssueSink::new(thr, threshold);
        for i in xs {
            sink.push(i.clone());
        }
        let (summary, it) = sink.finish(Regime::Emir, 1, 7, ts(), ts());
        let spilled = it.spill_path().is_some();
        let got: Vec<DqIssue> = it.collect();

        assert_eq!(got.len(), expected.len(), "length");
        assert!(
            got.windows(2)
                .all(|w| issue_cmp(&w[0], &w[1]) != Ordering::Greater),
            "non-decreasing under issue_cmp"
        );
        let mut a = ser(&got);
        let mut b = ser(&expected);
        a.sort();
        b.sort();
        assert_eq!(a, b, "same multiset of issues");
        assert_eq!(summary.issues_total as usize, xs.len(), "summary total");

        if !spilled {
            assert_eq!(
                ser(&got),
                ser(&expected),
                "no-spill ⇒ byte-identical to finalize"
            );
        }
    }

    #[test]
    fn empty() {
        assert_equiv(&[], &Thresholds::default(), 4);
    }

    #[test]
    fn single() {
        assert_equiv(
            &[mk("EMIR.B", Severity::High, "m")],
            &Thresholds::default(),
            4,
        );
    }

    #[test]
    fn below_threshold_no_spill_is_byte_identical() {
        let xs = vec![
            mk("EMIR.B", Severity::High, "b"),
            mk("EMIR.A", Severity::Warning, "z"),
            mk("EMIR.A", Severity::Warning, "a"),
        ];
        assert_equiv(&xs, &Thresholds::default(), 100);
    }

    #[test]
    fn exactly_threshold_then_more() {
        let xs: Vec<DqIssue> = (0..5)
            .map(|i| mk(&format!("EMIR.C{i}"), Severity::Info, &format!("m{i}")))
            .collect();
        assert_equiv(&xs, &Thresholds::default(), 2); // runs of 2,2 + residual 1
    }

    #[test]
    fn many_tiny_runs() {
        let xs: Vec<DqIssue> = (0..50)
            .map(|i| {
                mk(
                    &format!("EMIR.{:02}", 49 - i),
                    Severity::High,
                    &format!("m{i}"),
                )
            })
            .collect();
        assert_equiv(&xs, &Thresholds::default(), 1); // one run per issue
    }

    #[test]
    fn duplicates_and_equal_keys() {
        let xs = vec![
            mk("EMIR.X", Severity::High, "same"),
            mk("EMIR.X", Severity::Critical, "same"), // issue_cmp-equal, sev differs
            mk("EMIR.X", Severity::High, "same"),
            mk("EMIR.A", Severity::Info, "a"),
        ];
        assert_equiv(&xs, &Thresholds::default(), 2);
    }

    #[test]
    fn with_evidence() {
        let mut e = mk("EMIR.E", Severity::Warning, "ev");
        e.evidence = vec![crate::model::EvidenceItem {
            field: "uti".into(),
            before: Some("U1".into()),
            after: Some("U2".into()),
            source_line: Some(3),
        }];
        let xs = vec![e, mk("EMIR.A", Severity::Info, "a")];
        assert_equiv(&xs, &Thresholds::default(), 1);
    }

    #[test]
    fn non_empty_severity_overrides_applied_before_sort() {
        let mut thr = Thresholds::default();
        thr.severity_overrides
            .insert("EMIR.NOISY".into(), Severity::Warning);
        let xs = vec![
            mk("EMIR.NOISY", Severity::High, "n1"),
            mk("EMIR.NOISY", Severity::High, "n2"),
            mk("EMIR.A", Severity::Info, "a"),
        ];
        // exercised both no-spill and spill
        assert_equiv(&xs, &thr, 100);
        assert_equiv(&xs, &thr, 1);
        // override actually took effect
        let mut sink = SortedIssueSink::new(&thr, 100);
        for i in &xs {
            sink.push(i.clone());
        }
        let (_, it) = sink.finish(Regime::Emir, 1, 1, ts(), ts());
        let got: Vec<DqIssue> = it.collect();
        assert!(got
            .iter()
            .filter(|i| i.check_id == "EMIR.NOISY")
            .all(|i| i.severity == Severity::Warning));
    }

    #[test]
    fn spill_dir_removed_after_full_consumption() {
        let xs: Vec<DqIssue> = (0..10)
            .map(|i| mk("EMIR.A", Severity::High, &format!("m{i}")))
            .collect();
        let mut sink = SortedIssueSink::new(&Thresholds::default(), 1);
        for i in &xs {
            sink.push(i.clone());
        }
        let (_, it) = sink.finish(Regime::Emir, 1, 1, ts(), ts());
        let dir = it.spill_path().expect("spilled");
        assert!(dir.exists(), "spill dir present during iteration");
        let _drained: Vec<DqIssue> = it.collect();
        assert!(!dir.exists(), "spill dir removed after the iterator drops");
    }

    #[test]
    fn spill_dir_removed_after_early_drop() {
        let xs: Vec<DqIssue> = (0..10)
            .map(|i| mk("EMIR.A", Severity::High, &format!("m{i}")))
            .collect();
        let mut sink = SortedIssueSink::new(&Thresholds::default(), 1);
        for i in &xs {
            sink.push(i.clone());
        }
        let (_, it) = sink.finish(Regime::Emir, 1, 1, ts(), ts());
        let dir = it.spill_path().expect("spilled");
        assert!(dir.exists());
        drop(it); // dropped without consuming
        assert!(!dir.exists(), "spill dir removed on early drop");
    }

    #[test]
    fn spill_dir_raii_unit() {
        let dir = std::env::temp_dir().join(format!(
            "opendqi-spill-raii-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("run-000000.jsonl"), b"{}").unwrap();
        let guard = SpillDir { dir: dir.clone() };
        assert!(dir.exists());
        drop(guard);
        assert!(!dir.exists());
    }
}
