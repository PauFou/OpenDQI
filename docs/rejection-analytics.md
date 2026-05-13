# Rejection analytics

Phase 3 of the post-TR intelligence roadmap. `opendqi feedback
analytics` aggregates every feedback row in the SQLite history
store and produces an actionable rejection profile.

```bash
opendqi feedback analytics --store ./opendqi-history.db [--regime emir|sftr] --out ./rejection_analytics/
```

Outputs:

- `rejection_summary.json` — top rejection causes, repeated
  rejected UTIs, age buckets (0–1d / 1–7d / 7–30d / 30d+),
  rejected-then-accepted list, stale-open count.
- `rejections.csv` — flat dump of every row currently `open` or
  `stale` (resolved rows are intentionally excluded; they no longer
  need attention).
- `rejection_profile.yml` — human-readable pattern catalog with
  `suggested_check` placeholders; the user copies the patterns
  into their pre-submission check pack as appropriate.
- `rejection_report.html` — minimal HTML view of the same data.

## Detected patterns

### Top rejection causes

Histogram of `reason_code` values across the entire feedback table
(filterable by regime). The top-10 are surfaced in the summary and
profile.

### Repeated rejected UTIs

UTIs with **3 or more** rejection rows are listed explicitly. These
indicate either a stuck UTI that the firm keeps re-submitting in
the same broken state, or a TR-side validation that the firm has
not yet diagnosed.

### Stale open rejections

Rows with `status='open'` whose `status_set_at` is more than
**7 days** old are counted. The threshold is a compile-time
constant in v1; it will move to the Thresholds config when a real
profile emerges.

### Rejected-then-accepted

For each rejected UTI, the analytics path queries the store for a
later NEWT in `emir_records` whose `reporting_timestamp` is after
the rejection's `ingested_at`. When found, the row is included in
the summary with both timestamps — a useful audit trail showing
the firm did remediate the issue.

## Profile output

`rejection_profile.yml` is intentionally simple — it is **not** a
rules engine input today. It is a copy-paste catalogue:

```yaml
profile:
  generated_at: "2026-05-13T08:00:00Z"
  total_feedbacks: 142
  top_causes:
    - reason_code: "VAL01"
      count: 38
      suggested_check: "pre-submission rule on VAL01"
    - reason_code: "VAL12"
      count: 27
      suggested_check: "pre-submission rule on VAL12"
  repeated_rejected_utis:
    - uti: "U-STUCK-001"
      count: 5
```

The OpenDQI YAML rules engine (planned, not v1) will eventually
read this format directly.

## Design notes

- The analytics path is intentionally implemented **inline in the
  CLI runner** rather than through the existing `FeedbackCheck`
  trait. Modifying that trait's signature would touch the 8
  existing feedback checks for limited benefit; the analytics work
  is naturally aggregate, not row-by-row.
- Two new SQL aggregate queries land in `opendqi-store`:
  `count_feedbacks_by_reason` and `count_feedbacks_by_uti`. Both
  filter by optional regime and order descending by count.
- All thresholds (`REPEATED_REJECTION_THRESHOLD = 3`,
  `STALE_REJECTION_AGE_DAYS = 7`) are compile-time constants in
  `crates/opendqi-cli/src/commands/feedback.rs` — they will become
  config-driven in a later milestone.
