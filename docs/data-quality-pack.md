# Data Quality Pack (v0.17+)

The Data Quality Pack is OpenDQI's **committee-readable**
view on top of the granular check catalogue. Where the
per-row `DqIssue` stream is forensic (222 checks, 6
dimensions), the DQI pack rolls them up into **40
regulator-style indicators** — 24 EMIR + 16 SFTR — each with
`numerator / denominator / rate / threshold / status`, plus
drill-down evidence.

Same scan, two views — the granular issue stream is
**co-produced**, never replaced. `issues.csv` keeps carrying
the per-row defects ; `indicators.csv` + `evidence.csv` are
additional outputs.

## 30-second example (EMIR)

```bash
opendqi emir data-quality-pack \
  --tsr  examples/quickstart-emir/auth107-tsr.xml \
  --tar  examples/quickstart-emir/auth030-tar.xml \
  --feedback examples/quickstart-emir/auth092-feedback.xml \
  --as-of 2026-05-21 \
  --out  ./pack/
```

Output under `./pack/` :

| file | content |
|---|---|
| `report.html` | Coloured (green/amber/red) indicator table + the existing Top Issues / by-severity / by-dimension sections. |
| `summary.json` | Standard `ScanSummary` — unchanged shape since v0.10. |
| `issues.csv` | v1.0 stable 11-column granular issues (unchanged contract since v0.12). |
| `indicators.csv` | v1.0 stable 11 columns, one row per shipped DQI. |
| `evidence.csv` | v1.0 stable 7 columns, ≤ 20 evidence rows per DQI. |

The `as-of` flag pins the cutoff used by the two stale-data
indicators so the report is reproducible across calendar days.

## 30-second example (SFTR)

```bash
opendqi sftr data-quality-pack \
  --tsr  examples/sftr/tr_state/auth079-sample.xml \
  --tar  examples/sftr/tr_activity/auth052-tar-sample.xml \
  --reconciliation       examples/sftr/reconciliation/auth080-sample.xml \
  --missing-collateral   examples/sftr/missing_collateral/auth083-sample.xml \
  --msr                  examples/sftr/margin_state/auth085-sample.xml \
  --as-of 2026-05-21 \
  --out  ./sftr-pack/
```

Same 5 output files. v0.17 ships **16 SFTR indicators** across
the 5 input layers — TSR (`auth.079`) + TAR (`auth.052`) +
reconciliation (`auth.080`) + missing-collateral (`auth.083`)
+ MSR (`auth.085`). Each layer flag is optional ; indicators
whose source layer isn't provided self-report `not_applicable`.

## The 24 EMIR indicators

Grouped by source layer for readability ; the on-disk
`indicators.csv` is always sorted alphabetically by
`indicator_id`.

### TSR-only (5)

| ID | Dimension | Numerator | Denominator | Default thresholds (amber / red) |
|---|---|---|---|---|
| `DQI_ANOMALY_RATE` | accuracy | TSR records with ≥ 1 accuracy-flavoured granular issue (negative/abnormal/zero) | total TSR records | 5 % / 20 % |
| `DQI_DUPLICATE_REPORTS` | uniqueness | distinct UTIs appearing in ≥ 2 TSR rows | total TSR records | 0.5 % / 2 % |
| `DQI_LEI_MISSING` | completeness | TSR records with ≥ 1 missing/empty counterparty LEI | total TSR records | 1 % / 5 % |
| `DQI_VAL_MISSING` | completeness | outstanding TSR rows with no / zero valuation | outstanding TSR rows | 0.5 % / 2 % |
| `DQI_VAL_STALE` | timeliness | rows with valuation older than `max_valuation_age_business_days` vs `as_of` (**TARGET2** business days) | rows with `valuation_timestamp` set | 1 % / 5 % |

### MSR-only (2)

| ID | Dimension | Numerator | Denominator | Default thresholds (amber / red) |
|---|---|---|---|---|
| `DQI_COL_ALL_ZERO` | accuracy | MSR rows where all 4 margin fields are zero / NULL | MSR rows | 2 % / 10 % |
| `DQI_COL_STALE_STATE` | timeliness | rows with `state_as_of` older than `collateral_max_age_days` (**TARGET2** business days) | rows with `state_as_of` set | 5 % / 20 % |

### TSR + MSR cross-layer (2)

| ID | Dimension | Numerator | Denominator | Default thresholds (amber / red) |
|---|---|---|---|---|
| `DQI_COL_MISSING_STATE` | completeness | outstanding-collateralised TSR rows with no MSR row | outstanding-collateralised TSR rows | 1 % / 5 % |
| `DQI_VM_MISSING_FOR_CLEARED` | completeness | FCOL MSR rows reporting no `variation_margin_collected_current` | FCOL MSR rows | 1 % / 5 % |

### TAR-only (4)

| ID | Dimension | Numerator | Denominator | Default thresholds (amber / red) |
|---|---|---|---|---|
| `DQI_ERR_MISSING` | completeness | TAR records with no entity-responsible-for-reporting LEI | total TAR records | 1 % / 5 % |
| `DQI_NATURE_MISSING` | completeness | TAR records with no nature (FC/NFC/NFC+) classifier | total TAR records | 5 % / 20 % |
| `DQI_SECTOR_MISSING` | completeness | TAR records with no `corporate_sector` classifier | total TAR records | 5 % / 20 % |
| `DQI_TIM_REPORTING_LATE` | timeliness | gap > `max_reporting_delay_hours` | rows with both timestamps set | 5 % / 20 % |

### TAR — gated (2)

These two read raw_fields that are present in some EMIR
submissions but optional. They self-report
`status: not_applicable` when the field isn't mapped or never
appears non-empty.

| ID | Dimension | Numerator | Denominator | Default thresholds (amber / red) |
|---|---|---|---|---|
| `DQI_CONF_MISSING` | completeness | TAR rows with no `confirmation_timestamp` | TAR rows | 5 % / 20 % |
| `DQI_REC_STATUS_UNPAIRED` | consistency | rows tagged unpaired / unreconciled | rows with status set | 5 % / 20 % |

### Feedback layer — `auth.092` (2)

| ID | Dimension | Numerator | Denominator | Default thresholds (amber / red) |
|---|---|---|---|---|
| `DQI_REJ_RATE` | accuracy | rejected feedback records | total feedback records (proxy) | 1 % / 5 % |
| `DQI_REJ_REPEAT_UTI` | accuracy | distinct UTIs rejected ≥ 2× | distinct rejected UTIs | 0.5 % / 2 % |

### Cross-counterparty reconciliation — `auth.091` (7)

These 7 indicators read the EMIR reconciliation statistical
report (`auth.091`). They split into two families:
**aggregate stats** (pairing/reconciliation rates) computed
from `ReconStatsRecord` rollups, and **per-record
reconciliation** (field-level mismatch breakdowns) computed
from the `ReconciliationRecord` detail.

| ID | Dimension | Numerator | Denominator | Default thresholds (amber / red) |
|---|---|---|---|---|
| `DQI_PAIRING_RATE` | consistency | counter — *unpaired* records | total outstanding records reported in the stat | 20 % / 40 % |
| `DQI_RECONCILIATION_RATE` | consistency | unreconciled paired records | paired records | 15 % / 30 % |
| `DQI_UNPAIRED_TRADES_RATE` | consistency | per-trade records with `pairing_status = UNPAIRED` | total per-trade reconciliation records | 20 % / 40 % |
| `DQI_FIELD_MISMATCH_RATE` | consistency | reconciliation records with ≥ 1 mismatched field | total reconciliation records | 5 % / 20 % |
| `DQI_NOTIONAL_INCONSISTENT` | consistency | paired records flagged on the notional-amount criterion | paired records carrying the criterion | 5 % / 20 % |
| `DQI_MARGIN_INCONSISTENT_PRE_HAIRCUT` | consistency | paired records flagged on any IM/VM pre-haircut criterion | paired records carrying ≥ 1 such criterion | 5 % / 20 % |
| `DQI_MARGIN_INCONSISTENT_POST_HAIRCUT` | consistency | paired records flagged on any IM/VM post-haircut criterion | paired records carrying ≥ 1 such criterion | 5 % / 20 % |

**Wiring note (v0.16 honest scope)** — the new auth.091-derived
indicators ship in the core engine, the CLI flag and Python
keyword to feed `--recon-stats` / `--reconciliation` are
**not yet wired** on `opendqi emir data-quality-pack` ;
they self-report `not_applicable` until the follow-up commit
threads those inputs through both surfaces.

## The 16 SFTR indicators (v0.17)

Grouped by source layer for readability ; the on-disk
`indicators.csv` is always sorted alphabetically by
`indicator_id`. Every layer is optional ; missing layers
produce `not_applicable` placeholders so downstream
consumers always see the same 16 rows.

### TSR — `auth.079` (6)

3 v0.16 T2-layer indicators + 3 v0.17 SFTR-specific TSR
indicators.

| ID | Dimension | Numerator | Denominator | Default thresholds (amber / red) |
|---|---|---|---|---|
| `DQI_COLLATERAL_VALUE_MISSING_SFTR` | completeness | outstanding SFTR TSR rows with portfolio code OR collateral ISIN set but no `collateral_value` | outstanding SFTR TSR rows | 5 % / 20 % |
| `DQI_LOAN_VALUE_MISSING_SFTR` | completeness | outstanding SFTR TSR rows with no `loan_value` | outstanding SFTR TSR rows | 5 % / 20 % |
| `DQI_LOAN_VALUE_STALE_SFTR` | timeliness | rows with `state_as_of` older than `max_valuation_age_business_days` (**TARGET2** business days) | rows with `state_as_of` set | 5 % / 20 % |
| `DQI_HAIRCUT_ANOMALY_SFTR` | accuracy | rows where `haircut` is outside `[0.0, 1.0]` (regulatory bound per ESMA RTS 2019/356 Art. 4) | rows with `haircut` set | 0.5 % / 2 % |
| `DQI_LEI_MISSING_SFTR` | completeness | rows with ≥ 1 counterparty LEI missing/empty | total SFTR TSR rows | 1 % / 5 % |
| `DQI_UNDER_COLLATERALIZATION_SFTR` | accuracy | rows where `collateral_value × (1 − haircut) < loan_value` strictly | rows with `loan_value` + `collateral_value` + `haircut` all set | 0.5 % / 2 % |

### TAR — `auth.052` (1)

| ID | Dimension | Numerator | Denominator | Default thresholds (amber / red) |
|---|---|---|---|---|
| `DQI_TIM_REPORTING_LATE_SFTR` | timeliness | gap > `max_reporting_delay_hours` | TAR rows with both timestamps set | 5 % / 20 % |

### Reconciliation — `auth.080` (4)

The 4 SFTR reconciliation DQIs read the SFTR Reconciliation
Status Advice (`auth.080`) projected onto `ReconciliationRecord`
filtered defensively by `regime == Sftr`. v0.17 is per-trade
only — SFTR's `auth.080` doesn't ship per-CP cohort stats like
EMIR's `auth.091`.

| ID | Dimension | Numerator | Denominator | Default thresholds (amber / red) |
|---|---|---|---|---|
| `DQI_PAIRING_RATE_SFTR` | consistency | SFTR records flagged UNPAIRED / NOT_PAIRED / UNPR | SFTR records with `pairing_status` set | 5 % / 20 % |
| `DQI_RECONCILIATION_RATE_SFTR` | consistency | paired SFTR records flagged UNRECONCILED / NOT_RECONCILED / UNREC | SFTR records with `pairing_status == PAIRED` | 5 % / 20 % |
| `DQI_UNPAIRED_TRADES_RATE_SFTR` | consistency | SFTR records with `pairing_status` UNPAIRED | all SFTR records (None included as non-unpaired) | 5 % / 20 % |
| `DQI_FIELD_MISMATCH_RATE_SFTR` | consistency | SFTR records with `mismatched_fields` non-empty | SFTR records with `reconciliation_status` set | 5 % / 20 % |

`PAIRING_RATE_SFTR` excludes None-status records from the
denominator ; `UNPAIRED_TRADES_RATE_SFTR` counts them as
non-unpaired in the denominator. The two diverge when a
non-trivial subset lacks a pairing status — operational
signal that `auth.080` coverage is poor.

### Missing-collateral — `auth.083` (1)

| ID | Dimension | Numerator | Denominator | Default thresholds (amber / red) |
|---|---|---|---|---|
| `DQI_MCR_OPEN_REQUESTS_SFTR` | completeness | MCR records whose UTI is **not** in the SFTR TSR snapshot (or **all** MCR records when no TSR companion is provided — degraded mode) | MCR records with UTI populated | 5 % / 20 % |

### MSR — `auth.085` T3 margin (4)

The SFTR Margin Data Transaction State Report (`auth.085`) is
portfolio-level (indexed by `collateral_portfolio_code`, **not**
UTI) and restricted to **CCP-cleared SFTs** per ESMA scope. The
6 amount fields per portfolio (IM/VM posted/received + excess
collateral posted/received) drive the following 4 DQIs.

| ID | Dimension | Numerator | Denominator | Default thresholds (amber / red) |
|---|---|---|---|---|
| `DQI_T3_MARGIN_POSTED_MISSING_SFTR` | completeness | MSR records with **no** posted amount set (IM / VM / XcssColl posted all None) | MSR records with `collateral_portfolio_code` set | 5 % / 20 % |
| `DQI_T3_MARGIN_RECEIVED_MISSING_SFTR` | completeness | MSR records with **no** received amount set | MSR records with `collateral_portfolio_code` set | 5 % / 20 % |
| `DQI_T3_EXCESS_COLLATERAL_USE_SFTR` | accuracy | MSR records with `excess_collateral_posted > 0` OR `excess_collateral_received > 0` (SFTR-specific — flags TR-side reporting inflation or operational over-collateralisation) | MSR records with ≥ 1 amount set | 20 % / 50 % |
| `DQI_T3_MARGIN_STALE_SFTR` | timeliness | MSR records with `state_as_of` older than `max_valuation_age_business_days` (**TARGET2** business days) | MSR records with `state_as_of` AND ≥ 1 amount set | 5 % / 20 % |

The MSR layer also triggers 6 granular `SFTR.T3.*` per-record
checks (`IM/VM_POSTED/RECEIVED_MISSING` for partial-side
reporting, `MARGIN_NEGATIVE`, `MARGIN_CURRENCY_MISSING`) that
are co-produced into `issues.csv` alongside the aggregated
DQIs.

## Status mapping (common to all 40)

- `rate ≤ amber_threshold` → **green**
- `amber_threshold < rate ≤ red_threshold` → **amber**
- `rate > red_threshold` → **red**
- `denominator == 0` OR input layer not provided OR gated
  field unmapped → **not_applicable** (`rate = null`, no
  evidence)

## TARGET2 business-day calendar (v0.16+)

The two stale-data indicators (`DQI_VAL_STALE` +
`DQI_COL_STALE_STATE`) and the SFTR `DQI_LOAN_VALUE_STALE_SFTR`
compare timestamps against `as_of` using the **TARGET2** ECB
calendar : weekends are excluded, plus the 6
Eurosystem-published holidays per year (1 Jan, Good Friday,
Easter Monday, 1 May, 25 Dec, 26 Dec). The hardcoded calendar
covers **2025 → 2032** and is bumpable in
`crates/opendqi-core/src/business_days/target2_holidays.rs` ;
out-of-range dates fall back to weekend-only semantics with
an inline note. v0.15 used calendar days as a proxy ; v0.16
makes the cutoff regulator-aligned.

## Disclaimer — what a DQI is NOT

The vocabulary matters here because the consumers of this
output (committee, compliance officer, supervisor, audit
team) read each word literally.

- **A DQI is not a validation rule.** Validation rules are
  the per-row `EMIR.*` / `SFTR.*` checks (the 216-check
  catalogue) that flag *individual* defects. A DQI is an
  *aggregation* of one or more such defects into a single
  rate ; the rule is the row-level check, the indicator is
  the rollup.
- **A DQI is not a verdict of non-conformity.** A `red`
  status is an internal *alert* asking the firm to
  investigate — it is **not** a declaration that the firm
  has breached a regulation. The rate threshold beyond
  which a DQI turns `red` is configurable by the firm, not
  set by a regulator.
- **A DQI is an internal control indicator.** It exists to
  help the firm prioritise its own remediation work and
  produce a committee-readable view of data-quality trends
  over time.
- **OpenDQI computes internal data quality indicators. It
  does not certify regulatory compliance.** Compliance with
  EMIR / SFTR reporting obligations remains the firm's
  responsibility ; the firm should validate every output
  against applicable rules, internal controls, and
  professional advice.

In short: the DQI pack helps a firm *see* its data quality.
It does **not** *certify* it.

## Indicator details — EMIR

### `DQI_VAL_MISSING`

**Question**: *"What share of my outstanding trades are reported with no valuation?"*

- **Numerator**: TSR records where `valuation_amount` is
  `None` or `0`, AND `status` is not `MATURED` /
  `TERMINATED`, AND `termination_date` is unset.
- **Denominator**: same predicate but without the
  no-valuation filter (i.e. outstanding TSR rows).
- **Evidence**: top-20 offenders sorted by
  `(source_file, uti)` ascending — UTI + reporting
  counterparty + source file. `observed_value` is the
  serialised `valuation_amount` (empty when `None`).

### `DQI_VAL_STALE`

**Question**: *"How fresh are my valuations relative to today?"*

- **Numerator**: TSR records where the TARGET2 business-day
  gap from `valuation_timestamp` to `as_of` exceeds
  `max_valuation_age_business_days` (default `1`).
- **Denominator**: TSR records with `valuation_timestamp`
  set (rows without are excluded — already counted by
  `DQI_VAL_MISSING`).
- **Evidence**: oldest timestamps first.

### `DQI_COL_MISSING_STATE`

**Question**: *"Article 11 — for my collateralised outstanding trades, is the margining state actually reported?"*

- Mirrors the per-row `EMIR.COL.MISSING` check at the
  indicator level — same `compute_collateral_emir_issues`
  UTI-indexing pattern in `dq/collateral_audit.rs`.
- **Denominator**: outstanding TSR records with
  `collateral_portfolio_code` set.
- **Numerator**: same, minus those whose UTI is present in
  the MSR.
- **Evidence**: missing UTIs first, with portfolio code as
  `observed_value`.

### `DQI_COL_ALL_ZERO`

**Question**: *"Are some MSR rows actually empty (signalling a UCOL trade misreported or a margining gap)?"*

- **Numerator**: MSR rows where all 4 margin fields
  (`initial_margin_posted_current`,
  `initial_margin_collected_current`,
  `variation_margin_posted_current`,
  `variation_margin_collected_current`) are `None` or `0`.
- **Denominator**: all MSR rows.

### `DQI_COL_STALE_STATE`

**Question**: *"Are the margining snapshots up-to-date?"*

- Mirrors `EMIR.COL.STALE` at the indicator level. Same
  `collateral_max_age_days` threshold (default 1 TARGET2
  business day).
- **Numerator**: MSR rows where the business-day gap from
  `state_as_of` to `as_of` exceeds `collateral_max_age_days`.
- **Denominator**: MSR rows with `state_as_of` set.

### `DQI_VM_MISSING_FOR_CLEARED`

**Question**: *"For fully-collateralised trades, is variation margin actually reported?"*

- **Numerator**: MSR records with
  `collateralisation_type = FCOL` (fully collateralised) AND
  no `variation_margin_collected_current` value.
- **Denominator**: MSR records with `FCOL` collateralisation
  type.
- **Rationale**: more specific than `DQI_COL_MISSING_STATE`
  — focuses on the cleared/FCOL subset where VM reporting
  is most strictly expected.

### `DQI_ANOMALY_RATE`

**Question**: *"Across all the accuracy-flavoured checks, what share of records exhibit at least one anomaly?"*

- **Numerator**: TSR records where at least one of these
  fields trips the per-field accuracy heuristics
  (negative/abnormal/zero) :
  `notional_amount`, `price`, `maturity_date`,
  `execution_timestamp`, `event_timestamp`.
- **Denominator**: total TSR records.
- **Rationale**: rolls up the noise of multiple per-field
  `EMIR.ACC.*` granular checks into a single committee-grade
  number.

### `DQI_DUPLICATE_REPORTS`

**Question**: *"Are some UTIs reported more than once in the same TSR snapshot?"*

- **Numerator**: distinct UTIs that appear in ≥ 2 TSR rows
  in the snapshot.
- **Denominator**: total TSR records.
- **Evidence**: top-20 most-duplicated UTIs first ;
  `observed_value` carries the count.

### `DQI_LEI_MISSING`

**Question**: *"What share of my TSR records have a missing counterparty LEI?"*

- **Numerator**: TSR records with at least one of
  `reporting_counterparty`, `other_counterparty`,
  `entity_responsible_for_reporting` (LEI fields) empty
  or NULL.
- **Denominator**: total TSR records.

### `DQI_ERR_MISSING`

**Question**: *"What share of my TAR records carry an entity-responsible-for-reporting?"*

- **Numerator**: TAR records with no
  `entity_responsible_for_reporting` LEI.
- **Denominator**: total TAR records.

### `DQI_NATURE_MISSING`

**Question**: *"What share of my TAR records classify the counterparty (FC / NFC / NFC+)?"*

- **Numerator**: TAR records with no `nature` classifier.
- **Denominator**: total TAR records.

### `DQI_SECTOR_MISSING`

**Question**: *"What share of my TAR records carry a corporate-sector classifier?"*

- **Numerator**: TAR records with no `corporate_sector`.
- **Denominator**: total TAR records.

### `DQI_REJ_RATE`

**Question**: *"What share of what the TR sends back are rejections?"*

- **Numerator**: `auth.092` records with
  `feedback_type == Rejected`.
- **Denominator (honest scope)**: total feedback records.
  The *true* denominator would be "number of submissions",
  which `auth.092` alone doesn't carry. v0.16 still ships
  the proxy ; a real submission-count denominator is gated
  on a future store-backed workflow.

### `DQI_REJ_REPEAT_UTI`

**Question**: *"Are some UTIs chronically rejected (signalling a non-fixable structural issue)?"*

- **Numerator**: distinct UTIs that appear ≥ 2 times in
  `auth.092` with `feedback_type == Rejected`.
- **Denominator**: distinct UTIs with at least one
  rejection.
- **Evidence**: worst (most-rejected) first ;
  `observed_value` carries the count.

### `DQI_TIM_REPORTING_LATE`

**Question**: *"Are TAR submissions arriving within T+1?"*

- Mirrors `EMIR.TIM.LATE_REPORTING` at the indicator level.
- **Numerator**: TAR records where
  `(reporting_timestamp - execution_timestamp) > max_reporting_delay_hours`.
- **Denominator**: TAR records with both timestamps set.
- **Evidence**: biggest delay first ; `observed_value`
  carries the delay in hours (e.g. `"30h"`).

### `DQI_CONF_MISSING` (gated)

**Question**: *"Are confirmations being captured?"*

- Reads `raw_fields["confirmation_timestamp"]` on each TAR
  record.
- **Gated**: returns `status: not_applicable` if the field
  is not present in the mapping (CLI / Python detection)
  or never observed non-empty.

### `DQI_REC_STATUS_UNPAIRED` (gated)

**Question**: *"Of records carrying a TR-provided reconciliation status, how many are unpaired?"*

- Reads `raw_fields["reconciliation_status"]`. Token set
  matched (case-insensitive): `unpaired`, `not_paired`,
  `unrec`, `unreconciled`, `unmatched`.
- **Gated** like `DQI_CONF_MISSING`.

### `DQI_PAIRING_RATE` (auth.091)

**Question**: *"From the TR's perspective, what share of outstanding records are unpaired with the counterparty?"*

- **Numerator**: aggregate `unpaired` count reported in
  `auth.091`'s reconciliation statistics.
- **Denominator**: total outstanding count covered by the
  stat.
- **Source**: `ReconStatsRecord` rollups parsed from the
  reconciliation report.

### `DQI_RECONCILIATION_RATE` (auth.091)

**Question**: *"Of the trades the TR considers paired, what share are NOT reconciled field-by-field?"*

- **Numerator**: unreconciled paired records.
- **Denominator**: paired records (from `ReconStatsRecord`).

### `DQI_UNPAIRED_TRADES_RATE` (auth.091)

**Question**: *"At the per-trade level, what share are flagged unpaired?"*

- **Numerator**: per-trade `ReconciliationRecord`s where
  `pairing_status = UNPAIRED`.
- **Denominator**: total per-trade reconciliation records.

### `DQI_FIELD_MISMATCH_RATE` (auth.091)

**Question**: *"What share of the per-trade reconciliation records have at least one mismatched field?"*

- **Numerator**: `ReconciliationRecord`s with ≥ 1
  reconciled criterion in the `mismatch_fields` list.
- **Denominator**: total `ReconciliationRecord`s.

### `DQI_NOTIONAL_INCONSISTENT` (auth.091)

**Question**: *"Of paired records carrying a notional criterion, what share flag it as mismatched?"*

- **Numerator**: `ReconciliationRecord`s flagged on the
  notional-amount criterion.
- **Denominator**: `ReconciliationRecord`s carrying the
  notional criterion.
- **Evidence**: per-UTI breakdown with the firm-vs-CP
  notional values.

### `DQI_MARGIN_INCONSISTENT_PRE_HAIRCUT` (auth.091)

**Question**: *"Of paired records carrying pre-haircut IM/VM criteria, what share flag any of them as mismatched?"*

- **Numerator**: `ReconciliationRecord`s flagged on any of
  the pre-haircut IM/VM (posted/received) criteria.
- **Denominator**: records carrying ≥ 1 such criterion.

### `DQI_MARGIN_INCONSISTENT_POST_HAIRCUT` (auth.091)

Same shape as `DQI_MARGIN_INCONSISTENT_PRE_HAIRCUT` but on
the post-haircut IM/VM criteria. Both indicators reuse a
shared `criterion_mismatch_rate` helper so the
numerator/denominator semantics never drift.

## Indicator details — SFTR

### `DQI_LOAN_VALUE_MISSING_SFTR`

**Question**: *"What share of my outstanding SFTs report no loan value?"*

- **Numerator**: SFTR TSR records with `loan_value` `None`
  or `0`.
- **Denominator**: total SFTR TSR records.
- **SFTR mirror of `DQI_VAL_MISSING`**.

### `DQI_LOAN_VALUE_STALE_SFTR`

**Question**: *"How fresh is the state of my outstanding SFTs?"*

- **Numerator**: SFTR TSR records where the TARGET2
  business-day gap from `state_as_of` to `as_of` exceeds
  `max_valuation_age_business_days` (the same EMIR
  threshold is reused).
- **Denominator**: SFTR TSR records with `state_as_of` set.

### `DQI_COLLATERAL_VALUE_MISSING_SFTR`

**Question**: *"For SFTs reporting any collateral identifier, is the collateral value actually populated?"*

- **Numerator**: SFTR TSR records with
  `collateral_portfolio_code` OR `collateral_isin` set but
  `collateral_value` `None` or `0`.
- **Denominator**: SFTR TSR records with at least one
  collateral identifier set.

### `DQI_TIM_REPORTING_LATE_SFTR`

**Question**: *"Are SFTR TAR submissions arriving within T+1?"*

- **Numerator**: SFTR TAR (`auth.052`) records where the
  gap from `execution_timestamp` to `reporting_timestamp`
  exceeds `max_reporting_delay_hours`.
- **Denominator**: TAR records with both timestamps set.
- **SFTR mirror of `DQI_TIM_REPORTING_LATE`**.

### `DQI_HAIRCUT_ANOMALY_SFTR`

**Question**: *"What share of my SFTs report a haircut outside the regulatory range?"*

- **Numerator**: SFTR TSR records where `haircut < 0` OR
  `haircut > 1` (strict). The `[0, 1]` bound is regulatory
  (ESMA RTS 2019/356 Art. 4) — only the **rate** threshold
  on the DQI is configurable.
- **Denominator**: SFTR TSR records with `haircut` set.
- Rolls up the granular `SFTR.COMP.HAIRCUT_OUT_OF_RANGE`
  check.

### `DQI_LEI_MISSING_SFTR`

**Question**: *"What share of my SFTR records have a missing counterparty LEI?"*

- **Numerator**: SFTR TSR records where
  `reporting_counterparty` OR `other_counterparty` is empty
  / None. Both LEIs are mandatory in the SFTR XSD
  (`Counterparty39__1` requires `RptgCtrPty` + `OthrCtrPty`)
  so misses signal a parsing failure, a non-LEI natural
  person on the other-CP side, or a corrupt feed.
- **Denominator**: total SFTR TSR records.
- **SFTR mirror of `DQI_LEI_MISSING`**.

### `DQI_UNDER_COLLATERALIZATION_SFTR`

**Question**: *"Are some SFTs reporting collateral that — once the haircut is applied — doesn't cover the loan?"*

- **Numerator**: SFTR TSR records where
  `collateral_value × (1 − haircut) < loan_value` strictly.
- **Denominator**: SFTR TSR records with all 3 inputs
  (`loan_value`, `collateral_value`, `haircut`) populated ;
  records missing any of the 3 are out of scope (their gaps
  are flagged by the granular `SFTR.COMP.*_MISSING` checks).
- Flags either a mis-reported value (DQ defect — the
  intended signal) or a genuine under-collateralisation
  (credit-risk anomaly, worth surfacing nonetheless).
- Evidence includes the computed shortfall
  (`loan − effective_collateral`) for triage.

### `DQI_PAIRING_RATE_SFTR` (auth.080)

**Question**: *"Of SFTR records whose pairing status the TR sent us, what share are unpaired?"*

- **Numerator**: SFTR `ReconciliationRecord`s with
  `pairing_status` ∈ `{UNPAIRED, NOT_PAIRED, UNPR}`
  (case-insensitive).
- **Denominator**: SFTR `ReconciliationRecord`s with
  `pairing_status` populated (excludes None).
- Sister to `DQI_UNPAIRED_TRADES_RATE_SFTR` which uses
  **total records** (including None) as denominator —
  divergence between the two signals poor `auth.080`
  coverage.

### `DQI_RECONCILIATION_RATE_SFTR` (auth.080)

**Question**: *"Of paired SFTs, what share are flagged unreconciled at the field level?"*

- **Numerator**: SFTR `ReconciliationRecord`s with
  `reconciliation_status` ∈ `{UNRECONCILED, NOT_RECONCILED,
  UNREC}`.
- **Denominator**: SFTR `ReconciliationRecord`s with
  `pairing_status == PAIRED` — field-level reconciliation
  is meaningless without a pair.

### `DQI_UNPAIRED_TRADES_RATE_SFTR` (auth.080)

**Question**: *"At the per-trade level, what share of all SFTs flagged unpaired?"*

- **Numerator**: SFTR records with `pairing_status` UNPAIRED.
- **Denominator**: **all** SFTR `ReconciliationRecord`s
  (None-status counted as non-unpaired).
- Sister to `DQI_PAIRING_RATE_SFTR` ; see that DQI for the
  denominator-floor difference and the operational signal.

### `DQI_FIELD_MISMATCH_RATE_SFTR` (auth.080)

**Question**: *"What share of paired SFTR records carry at least one mismatched field?"*

- **Numerator**: SFTR `ReconciliationRecord`s with
  `mismatched_fields` non-empty.
- **Denominator**: SFTR `ReconciliationRecord`s with
  `reconciliation_status` populated.
- Per-criterion EMIR DQIs (`DQI_NOTIONAL_INCONSISTENT`,
  `DQI_MARGIN_INCONSISTENT_*`) are not mirrored —
  `auth.080`'s `MsmtchFlds` is a flat list, not split by
  criterion family.

### `DQI_MCR_OPEN_REQUESTS_SFTR` (auth.083)

**Question**: *"For each Missing Collateral Request the TR sends us, has the requested UTI shown up with collateral in the latest TSR snapshot?"*

- **Numerator**:
  - With TSR companion (`--tsr` + `--missing-collateral`) :
    MCR records whose UTI is NOT in the TSR snapshot.
  - **Without** TSR companion (`--missing-collateral`
    alone) : **all** MCR records (degraded mode → 100 %
    red). The `description` field surfaces this mode.
- **Denominator**: MCR records with UTI populated (records
  without UTI are out of scope ; the granular
  `SFTR.MCR.REQUEST_WITHOUT_UTI` check covers them).

### `DQI_T3_MARGIN_POSTED_MISSING_SFTR` (auth.085)

**Question**: *"What share of CCP-cleared portfolios report no margin posted at all?"*

- **Numerator**: SFTR MSR records where **all 3** posted-side
  amounts (`initial_margin_posted`, `variation_margin_posted`,
  `excess_collateral_posted`) are None.
- **Denominator**: MSR records with `collateral_portfolio_code`
  set (the mandatory portfolio identifier in `auth.085`).

### `DQI_T3_MARGIN_RECEIVED_MISSING_SFTR` (auth.085)

Symmetric to `DQI_T3_MARGIN_POSTED_MISSING_SFTR` on the
received side. Numerator counts MSR records with **all 3**
received-side amounts None.

### `DQI_T3_EXCESS_COLLATERAL_USE_SFTR` (auth.085)

**Question**: *"What share of CCP-cleared portfolios are reporting any excess collateral on either side?"*

- **Numerator**: MSR records with
  `excess_collateral_posted > 0` OR
  `excess_collateral_received > 0`.
- **Denominator**: MSR records with ≥ 1 amount populated.
- **SFTR-specific** — `XcssColl*` has no EMIR auth.109
  equivalent. A high rate flags either TR-side reporting
  inflation (excess reported on every portfolio
  regardless of actual margining) or operational
  over-collateralisation (capital tied up in excess
  collateral above the requirement). Threshold defaults are
  looser than the strict completeness DQIs (20 % / 50 %).

### `DQI_T3_MARGIN_STALE_SFTR` (auth.085)

**Question**: *"How fresh is the CCP-cleared margin state vs `as_of`?"*

- **Numerator**: MSR records where the TARGET2 business-day
  gap from `state_as_of` to `as_of` exceeds
  `max_valuation_age_business_days`.
- **Denominator**: MSR records with `state_as_of` set AND
  ≥ 1 amount populated (records without amounts are out
  of scope ; their completeness gaps are flagged by the
  posted/received MISSING DQIs above).
- Reuses the same threshold key as
  `DQI_LOAN_VALUE_STALE_SFTR` /
  `DQI_VAL_STALE` / `DQI_COL_STALE_STATE`.

## Threshold configuration

Override per-indicator via YAML config :

```yaml
# my-thresholds.yml
dqi:
  DQI_VAL_MISSING:
    amber: 0.001   # 0.1 %
    red:   0.005   # 0.5 %
  DQI_VAL_STALE:
    amber: 0.005
    red:   0.02
  DQI_LOAN_VALUE_STALE_SFTR:
    amber: 0.01
    red:   0.05
  # other indicators inherit the shipped defaults
```

Use it :

```bash
opendqi emir data-quality-pack ... --config my-thresholds.yml
opendqi sftr data-quality-pack ... --config my-thresholds.yml
```

Missing indicator entries fall back to the shipped
defaults from `opendqi_core::default_dqi_thresholds()`,
then to the loose
`DqiThresholdPair::default { amber: 0.05, red: 0.20 }`.

## Python API

### EMIR

```python
import opendqi

result = opendqi.emir.data_quality_pack(
    tsr="auth107-tsr.xml",       # or pyarrow.Table
    tar="auth030-tar.xml",       # or pyarrow.Table
    msr="auth109-msr.xml",       # or pyarrow.Table
    feedback="auth092.xml",      # or pyarrow.Table
    mar="auth108-mar.xml",       # paths-only in v0.15
    mappings={                   # required when an input is a pyarrow.Table
        "tsr": {"uti": "TradeUti", "status": "Status", ...},
    },
    as_of="2026-05-21",          # defaults to today (UTC)
)

# v1.0 stable Arrow tables
result.indicators   # pyarrow.Table, 24 rows × 11 cols
result.evidence     # pyarrow.Table, ≤ 480 rows × 7 cols
result.issues       # pyarrow.Table, granular (same contract as v0.12+)

# dict shaped like summary.json
result.summary
```

### SFTR (v0.17+)

```python
import opendqi

result = opendqi.sftr.data_quality_pack(
    tsr="auth079-tsr.xml",                 # 6 TSR DQIs
    tar="auth052-tar.xml",                 # 1 TAR DQI
    reconciliation="auth080.xml",          # 4 recon DQIs (v0.17)
    missing_collateral="auth083.xml",      # 1 MCR DQI (v0.17)
    msr="auth085.xml",                     # 4 T3 DQIs + 6 SFTR.T3.* granular checks (v0.17)
    as_of="2026-05-21",
)
result.indicators   # pyarrow.Table, 16 rows × 11 cols
result.issues       # includes SFTR.T3.* granular checks when --msr provided
```

**v0.17 honest scope** — the SFTR DQI pack is still
**paths-only**. Arrow converters for `SftrTrStateRecord` /
`SftrMarginStateRecord` / `ReconciliationRecord` /
`MissingCollateralRecord` are scheduled for v0.18 ;
`opendqi.emir.data_quality_pack`'s dual `pyarrow.Table`
support has no equivalent on the SFTR side yet.

## Spark API (EXPERIMENTAL)

```python
import opendqi.spark.emir

result = opendqi.spark.emir.data_quality_pack(
    tsr=spark_df_tsr,            # collected at the driver
    feedback="auth092.xml",      # str paths still OK (mix-and-match)
    mappings={"tsr": {"uti": "TradeId", "status": "Status"}},
    as_of="2026-05-21",
)
# Same PyDqiPackResult as the core function.
```

**Honest** :
- Collect-then-call ; does not scale beyond driver-RAM.
- Native partition-aware joins = future work.
- `FutureWarning` emitted on every call.
- PySpark is optional : `pip install opendqi[spark]`.

## v1.0 Arrow schemas (locked since v0.15.0)

The 11-column `indicators` schema and 7-column `evidence`
schema are **frozen** — every expansion since v0.15.0
(v0.16's 14 new EMIR DQIs, v0.17's 12 new SFTR DQIs) adds
*rows*, not *columns*. Any breaking change requires a major
version bump of the bindings.

### `indicators` schema (11 cols)

| column | type | nullable |
|---|---|---|
| `indicator_id` | Utf8 | false |
| `regime` | Utf8 | false |
| `dimension` | Utf8 | false |
| `table_scope` | Utf8 | false |
| `numerator` | UInt64 | false |
| `denominator` | UInt64 | false |
| `rate` | Float64 | true |
| `threshold_amber` | Float64 | true |
| `threshold_red` | Float64 | true |
| `status` | Utf8 | false |
| `description` | Utf8 | false |

### `evidence` schema (7 cols)

| column | type | nullable |
|---|---|---|
| `indicator_id` | Utf8 | false |
| `uti` | Utf8 | false |
| `counterparty` | Utf8 | true |
| `asset_class` | Utf8 | true |
| `source_file` | Utf8 | true |
| `observed_value` | Utf8 | true |
| `explanation` | Utf8 | false |

Both contracts are pinned by parity tests in
`crates/opendqi-py/tests/test_data_quality_pack.py` (EMIR)
and `test_sftr_data_quality_pack.py` (SFTR) against the
on-disk CLI goldens.

## SFTR message-to-layer mapping (v0.17 corrected)

OpenDQI v0.17 ships the **full SFTR DQI surface** across 5
ISO 20022 messages. The earlier v0.16 docs mistakenly treated
T3 margin as "inline in `auth.079`" — verification against
the real ESMA XSDs (March 2023 release, v1.1.0–v1.2.0)
proved this wrong : T3 margin amounts live in a **separate**
message `auth.085` (a dedicated CCP-cleared-only MSR).

| Message | Official ESMA name | Direction | Powers (v0.17 DQI prefix) |
|---|---|---|---|
| `auth.052` | `SecuritiesFinancingReportingTransactionReport` | Firm → TR | `DQI_TIM_REPORTING_LATE_SFTR` |
| `auth.079` | `SecuritiesFinancingReportingTransactionStateReport` | TR → firm / NCA | `DQI_LOAN_VALUE_*_SFTR`, `DQI_COLLATERAL_VALUE_MISSING_SFTR`, `DQI_HAIRCUT_ANOMALY_SFTR`, `DQI_LEI_MISSING_SFTR`, `DQI_UNDER_COLLATERALIZATION_SFTR` (6 TSR DQIs total) |
| `auth.080` | `SecuritiesFinancingReportingReconciliationStatusAdvice` | TR → firm | `DQI_PAIRING_RATE_SFTR`, `DQI_RECONCILIATION_RATE_SFTR`, `DQI_UNPAIRED_TRADES_RATE_SFTR`, `DQI_FIELD_MISMATCH_RATE_SFTR` (4 recon DQIs) |
| `auth.083` | `SecuritiesFinancingReportingMissingCollateralRequest` | TR → firm | `DQI_MCR_OPEN_REQUESTS_SFTR` |
| `auth.085` | `SecuritiesFinancingReportingMarginDataTransactionStateReport` | TR → firm / NCA | `DQI_T3_MARGIN_POSTED_MISSING_SFTR`, `DQI_T3_MARGIN_RECEIVED_MISSING_SFTR`, `DQI_T3_EXCESS_COLLATERAL_USE_SFTR`, `DQI_T3_MARGIN_STALE_SFTR` (4 T3 margin DQIs) + 6 SFTR.T3.* granular checks |

`auth.085` is **portfolio-level** (indexed by
`collateral_portfolio_code`, not UTI) and **restricted to
CCP-cleared SFTs** per ESMA scope, exposing 6 amount fields
per portfolio : IM / VM / excess collateral × posted /
received. SFTR does NOT have a pre-haircut vs post-haircut
split on margin amounts (unlike EMIR auth.109 which carries
both), so the v0.17 SFTR T3 indicator family is intentionally
narrower than its EMIR counterpart.

See [`docs/iso20022-sftr.md`](iso20022-sftr.md) for per-message
XSD path documentation, and [`docs/auth-messages/`](auth-messages/)
for per-message reference pages.

## What v0.17 deliberately does NOT do

- **CLI/Python wiring of `auth.091` for the 7 cross-CP EMIR
  DQIs**. The computers ship but the `--recon-stats` /
  `--reconciliation` flags are not yet exposed on `opendqi
  emir data-quality-pack` ; those 7 indicators self-report
  `not_applicable` until a follow-up commit (carry-over
  from v0.16 honest scope — not blocking v0.17 ship).
- **`pyarrow.Table` dual-input on the SFTR DQI pack**
  (v0.18+). The Arrow converters for `SftrTrStateRecord` /
  `SftrMarginStateRecord` / `ReconciliationRecord` /
  `MissingCollateralRecord` are not yet implemented — the
  SFTR side stays **paths-only** vs EMIR which accepts
  `pyarrow.Table` on `tsr` / `tar` / `msr` / `feedback`.
- **DQI history / trend tracking via the SQLite store**
  (v0.18+).
- **A submission-count-aware `DQI_REJ_RATE` denominator**
  (gated on the store workflow).
- **Native partition-aware Spark** (Spark stays
  collect-then-call / experimental). The Polars LazyFrame
  fast path has a documented dtype round-trip caveat on
  the SFTR side (see `crates/opendqi-py/tests/
  test_sftr_polars.py`).
- **Threshold profile presets** (one YAML override at a
  time today).
