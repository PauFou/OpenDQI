# Consolidated `tr-audit`

Phase 4 of the post-TR intelligence roadmap. `opendqi emir tr-audit`
ingests a TAR (`auth.030`), a TSR (`auth.107`), and a feedback file
(`auth.092`) together, runs every layer's checks, plus 3 cross-layer
coherence checks, and writes a single consolidated report.

```bash
opendqi emir tr-audit \
  --tar <auth.030.xml | dir> \
  --tsr <auth.107.xml> \
  --feedback <auth.092.xml> \
  [--store <db>] \
  --out <dir>
```

Outputs:

- `summary.json` — regime-uniform scan summary aggregating issue
  counts by severity and dimension over all layers.
- `tr_audit_issues.csv` — flat dump of every issue produced by any
  layer, sorted deterministically.
- `tr_audit_report.html` — single HTML view of the combined audit.

`--store` is optional. When set, the TAR records are persisted (via
the standard scan path), lifecycle checks run against the
accumulated history, and the prior submission history feeds the
feedback / TSR cross-references.

## Layers run by `tr-audit`

1. **Single-batch on TAR records** — the 89 EMIR `default_checks()`
   plus the 8 cross-batch `default_lifecycle_checks()` when the
   store is open.
2. **TSR layer** — the 7 EMIR.TST.* checks against the TSR records.
3. **Rejection layer** — the 4 EMIR.FBK.* checks against the
   feedback records.
4. **Activity layer** — the 5 EMIR.TRA.* checks against the TAR
   records, with the TSR passed as the optional companion to
   activate `NEWT_NOT_IN_TSR`.
5. **Cross-layer coherence** — the 3 new EMIR.AUD.* checks (below).

## Cross-layer coherence checks

| Check ID | Severity | What it detects |
|---|---|---|
| `EMIR.AUD.NEWT_IN_TAR_NOT_IN_TSR` | High | UTI is `NEWT`'d in the TAR but absent from the TSR — strong signal that the submission was not accepted by the TR (overlaps with `EMIR.TRA.NEWT_NOT_IN_TSR` when both are run; both surface the gap). |
| `EMIR.AUD.OUTSTANDING_IN_TSR_NOT_IN_TAR` | Warning | UTI is outstanding in the TSR but no record appears in the TAR for this period — may be a legitimate older outstanding trade, or evidence that the firm is no longer reporting against an active UTI. |
| `EMIR.AUD.REJECTED_BUT_OUTSTANDING_IN_TSR` | Critical | UTI appears in the feedback as `Rejected` yet is listed as outstanding in the TSR — major TR-side inconsistency that demands escalation. |

## Design notes

- The 3 coherence checks are implemented **inline in the CLI
  runner** rather than through a dedicated `TrAuditCheck` trait —
  they are too specific to the three-way fan-in to warrant a
  reusable abstraction.
- All layers' outputs are merged into a single `tr_audit_issues.csv`
  to make the consolidated remediation list trivially scriptable.
  The deterministic sort key `(check_id, source_file, record_id)`
  is preserved so diffs across runs stay clean.
- `summary.json` uses the standard `ScanSummary` shape — the
  consumer can grep by check_id family (`EMIR.COMP`, `EMIR.TST`,
  `EMIR.FBK`, `EMIR.TRA`, `EMIR.AUD`, …) to slice by layer.

## SFTR (`auth.052` + `auth.079` + `auth.080`)

The same consolidated audit is available for SFTR:

```bash
opendqi sftr tr-audit \
  --tar examples/sftr/tr_activity/auth052-tar-sample.xml \
  --tsr examples/sftr/tr_state/auth079-sample.xml \
  --feedback examples/sftr/feedback/auth080-sample.xml \
  [--store <db>] --out <dir>
```

It runs every SFTR layer (single-batch, lifecycle, TSR, TAR, feedback)
and adds 3 inline cross-layer `SFTR.AUD.*` checks mirroring the EMIR
equivalents: `NEWT_IN_TAR_NOT_IN_TSR`, `OUTSTANDING_IN_TSR_NOT_IN_TAR`,
`REJECTED_BUT_OUTSTANDING_IN_TSR`. Output files are
`summary.json`, `tr_audit_issues.csv`, `tr_audit_report.html`.
