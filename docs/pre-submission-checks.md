# Pre-submission checks (EMIR.PSC.*)

OpenDQI closes the loop between post-TR rejection analytics and the
next pre-submission scan. The `opendqi feedback analytics` workflow
exports a `rejection_profile.yml` summarising the TR's rejection
patterns; `opendqi emir scan --rejection-profile <yml>` consumes that
file and runs the `EMIR.PSC.*` check family to flag records that
match historical rejection patterns **before** they go back to the
TR.

## Workflow

```
[TR rejects 250 reports]
        │
        ▼
opendqi emir feedback ./auth092.xml --store ./hist.db --out ./fb/
        │
        ▼
[feedbacks table in SQLite]
        │
        ▼
opendqi feedback analytics --store ./hist.db --out ./analytics/
        │           writes:
        │             • rejection_profile.yml  ← top causes + repeated UTIs
        │             • rejection_summary.json
        │             • rejection_report.html
        ▼
opendqi emir scan ./submissions.csv \
  --mapping mapping.yml \
  --rejection-profile ./analytics/rejection_profile.yml \
  --out ./scan/
        │
        ▼
[scan emits EMIR.PSC.* issues for risky records]
```

## Checks (2)

| Check ID | Severity | Dimension | Description |
|---|---|---|---|
| `EMIR.PSC.REPEATED_REJECTION` | high | consistency | Record's UTI appears in `profile.repeated_rejected_utis` — the TR has already rejected this UTI ≥ N times in the analytics window. Evidence: prior rejection count. |
| `EMIR.PSC.LIKELY_REJECTION_PATTERN` | warning | validity | Record matches a built-in predicate associated with a `profile.top_causes[].suggested_check` (e.g. `EMIR.COMP.NOTIONAL_CURRENCY_MISSING`, `EMIR.COMP.UTI_MISSING`). Message cites the rank (#1, #2, ...) and historical share. |

The `LIKELY_REJECTION_PATTERN` check ships with a small built-in
mapping from `suggested_check` IDs to record predicates. Currently
mapped:

- `EMIR.COMP.UTI_MISSING`
- `EMIR.COMP.NOTIONAL_CURRENCY_MISSING`
- `EMIR.COMP.VALUATION_MISSING`
- `EMIR.COMP.COUNTERPARTY_1_MISSING`
- `EMIR.COMP.COUNTERPARTY_2_MISSING`
- `EMIR.ACC.NEGATIVE_NOTIONAL`
- `EMIR.ACC.ZERO_NOTIONAL`

Adding a new entry is a single match arm in
`crates/opendqi-core/src/dq/pre_submission/likely_rejection_pattern.rs`.

## `rejection_profile.yml` format

`opendqi feedback analytics` writes the profile under a top-level
`profile:` key:

```yaml
profile:
  generated_at: "2026-05-14T12:00:00+00:00"
  total_feedbacks: 250
  top_causes:
    - reason_code: "VAL01"
      count: 85
      suggested_check: "EMIR.COMP.NOTIONAL_CURRENCY_MISSING"
    - reason_code: "VAL12"
      count: 60
      suggested_check: "EMIR.COMP.UTI_MISSING"
    - ...
  repeated_rejected_utis:
    - uti: "UTI-CHRONIC-001"
      count: 7
    - ...
```

`reason_code` values that don't map to a canonical check ID are
serialised with a sentinel `suggested_check: "(no canonical check
for VALXX)"` and skipped by the PSC engine. Extend the mapping
table in `crates/opendqi-cli/src/commands/feedback.rs:suggested_check_for_reason`.

## Why this matters

A naive scan re-validates a submission against a static rule book.
With the rejection profile, OpenDQI surfaces records that *look like*
the kinds of submissions that the TR has rejected from this firm
before — even when those records pass every other check. Critical
for back-office teams who want to break the cycle of submitting
known-problematic UTIs.

## Bundled fixture

[`examples/emir/rejection_profile/sample.yml`](../examples/emir/rejection_profile/sample.yml)
encodes a synthetic profile (3 top causes, 2 repeated UTIs).
Combined with any EMIR scan it triggers both PSC checks. See the
integration test
`crates/opendqi-core/tests/pre_submission_integration.rs` for the
canonical end-to-end example.
