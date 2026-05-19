# Collateral audit — EMIR TSR ↔ MSR cross-reference

`opendqi emir collateral-audit` cross-references the **Trade State
Report** (TSR, `auth.107`) against the **Margin State Report** (MSR,
`auth.109`) to surface the EMIR **Article 11 collateral obligation**:
for every outstanding TSR derivative, is there a linkable, non-zero,
timely margin-state snapshot in the MSR?

Unlike confirmation and valuation (which OpenDQI assesses on the
single-message submission and TSR scans), the collateral obligation
straddles two distinct ESMA messages — hence its own subcommand.

## Command

```bash
opendqi emir collateral-audit \
  --tsr ./auth.107.xml \
  --msr ./auth.109.xml \
  --out ./report/
# optional:
#   --config thresholds.yml   # override emir_rmt.collateral_max_age_days
#   --store ./history.db      # reserved for future cross-batch enrichments
#   --email-config smtp.yml   # email the report to the configured recipients
```

Args:

- `--tsr` — path to one EMIR TSR XML file (`auth.107.001.01`).
- `--msr` — path to one EMIR MSR XML file (`auth.109.001.01`).
- `--out` — directory where the report trio is written.
- `--config` — optional YAML `Thresholds` override. The relevant knob
  for this command is `emir_rmt.collateral_max_age_days` (default
  **1** — Article 11 daily margining expectation).
- `--store` — accepted for forward compatibility; currently unused
  (the command is single-snapshot today; future cross-batch
  enrichments will read it).
- `--email-config` — optional SMTP YAML; when set, the
  `collateral_audit_report.html` is emailed.

## Outputs

| File | Content |
|---|---|
| `summary.json` | Regime, files & TSR-record counts, issue totals by severity/dimension, quality score, started/finished timestamps. |
| `collateral_audit_issues.csv` | One row per issue (the two `EMIR.COL.*` checks plus any parse-time format issues from either XML). |
| `collateral_audit_report.html` | Human-readable summary, with the top issues. |

## Checks

| Check ID | Dim | Severity | Trigger |
|---|---|---|---|
| `EMIR.COL.MISSING` | Completeness | High | Outstanding TSR derivative with a non-empty UTI for which: (a) **no MSR row** matches by UTI, or (b) every one of the four current IM/VM (posted/collected) amounts is absent or zero across all matching MSR rows. |
| `EMIR.COL.STALE` | Timeliness | Warning | Linked, non-zero MSR snapshot whose most-recent `state_as_of` is older than `emir_rmt.collateral_max_age_days` (default **1**) calendar day(s) vs the TSR `state_as_of` (fallback: `now`). |

**Join key.** Always `uti` (1:1 primary). Both `TrStateRecord` and
`MarginStateRecord` carry it. Multiple MSR rows for the same UTI
(e.g. portfolio splits) are aggregated: the trade is MISSING only
if **every** matching row is all-zero / absent; for staleness the
**most-recent** matching `state_as_of` is used.

**TSR rows without a UTI are skipped** (cannot cross-reference);
that gap is already covered by the single-message TSR checks
(`EMIR.TST.*`) — no double-flagging.

## Honest scoping caveat

`TrStateRecord` (auth.107) does **not** carry a clearing-status flag
in the ESMA usage guideline OpenDQI consumes, so this check applies
to every *outstanding* TSR derivative. It is therefore a
**data-quality signal** — "TSR outstanding derivative without
linkable / fresh MSR margin state" — not a definitive verdict of
non-compliance with the Article 11 non-cleared collateral obligation
(consistent with the rest of OpenDQI: data quality, not certification).

## Threshold

```yaml
# thresholds.yml (snippet)
emir_rmt:
  collateral_max_age_days: 1   # default — Art.11 daily margining
```

Set to a larger value to relax the staleness gate (e.g. inter-day
runs against a previous-business-day MSR).

## Related

- [`docs/emir-risk-mitigation.md`](emir-risk-mitigation.md) — the
  full Article 11 picture, including the **obligation × {missing,
  timely}** matrix this command completes.
- [`docs/auth-messages/emir-auth107.md`](auth-messages/emir-auth107.md)
  and [`docs/auth-messages/emir-auth109.md`](auth-messages/emir-auth109.md) —
  field-by-field parser extraction maps.
- [`docs/emir-checks.md`](emir-checks.md) — the global EMIR catalog
  count (150 checks, of which the 2 `EMIR.COL.*` here).
