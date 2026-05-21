# DQI catalogue — mapping vs ESMA-style supervisory DQ indicators

This document maps the **OpenDQI Data Quality Pack** against
the supervisory-style data quality indicators commonly used
by NCAs and ESMA to monitor EMIR / SFTR reporting quality at
the Trade Repository level. The goal is to make it explicit
which standard DQ concepts the pack covers natively, which
are covered indirectly by the granular checks, and which are
intentionally out of scope.

The reference indicators are framed in **business / ESMA-public
terms** (pairing rate, reconciliation rate, missing valuation
rate, etc. — see ESMA EMIR / SFTR Q&A and the public DQ
dashboards). OpenDQI's coverage is intentionally a subset :
we ship the indicators that are stable, generalisable, and
computable from the public `auth.*` ISO 20022 message subset
the engine already parses.

## EMIR mapping (v0.16+)

Legend :
- 🟢 **DQI** = directly covered by a Data Quality Pack indicator (one row in `indicators.csv`)
- 🔵 **Granular** = covered at the per-row check level (one or more rows in `issues.csv`, no rolled-up indicator)
- ⚪ **Out of scope** = intentionally not covered (documented limitations)

### Pairing & reconciliation (TR-side, requires `auth.091` recon stats)

| Standard concept | Source layer | OpenDQI coverage | Indicator / Check ID |
|---|---|---|---|
| Trade-level pairing rate | `auth.091` | 🟢 DQI (v0.16) | `DQI_PAIRING_RATE_TCTN` |
| Position-level pairing rate | `auth.091` | 🟢 DQI (v0.16) | `DQI_PAIRING_RATE_POSI` |
| Field-by-field reconciliation rate | `auth.091` | 🟢 DQI (v0.16) | `DQI_RECONCILIATION_RATE` |
| Outstanding-count reconciliation (firm vs counterparty) | `auth.107` + `auth.091` | 🟢 DQI (v0.16) | `DQI_COUNT_RECONCILIATION` |
| Pairing rate trend (period-over-period) | `auth.091` × N | 🔵 Granular | `EMIR.RST.PAIRING_RATE_TREND_DOWN` |
| Unpaired trade detection | `auth.091` / `auth.107` | 🟢 DQI + 🔵 Granular | `DQI_REC_STATUS_UNPAIRED` + `EMIR.REC.UNPAIRED_TRADE` |

### Margin & notional cross-counterparty consistency (`auth.091` per-field)

| Standard concept | Source layer | OpenDQI coverage | Indicator ID |
|---|---|---|---|
| IM/VM consistency (pre-haircut) — firm vs counterparty | `auth.091` | 🟢 DQI (v0.16) | `DQI_MARGIN_INCONSISTENT_PRE_HAIRCUT` |
| IM/VM consistency (post-haircut) | `auth.091` | 🟢 DQI (v0.16) | `DQI_MARGIN_INCONSISTENT_POST_HAIRCUT` |
| Notional consistency — firm vs counterparty | `auth.091` | 🟢 DQI (v0.16) | `DQI_NOTIONAL_INCONSISTENT` |
| Per-record margin amounts (negative / zero / imbalance) | `auth.109` MSR | 🔵 Granular | `EMIR.MSR.INITIAL_MARGIN_NEGATIVE`, `…VARIATION_MARGIN_NEGATIVE`, `…IM_POSTED_VS_COLLECTED_IMBALANCE`, `…MARGIN_MISSING_FOR_OUTSTANDING` |

### Valuation quality (`auth.107` TSR)

| Standard concept | OpenDQI coverage | Indicator ID |
|---|---|---|
| Missing valuation on outstanding trades | 🟢 DQI (v0.15) | `DQI_VAL_MISSING` |
| Stale valuation (older than configured threshold) | 🟢 DQI (v0.15, **TARGET2 business days since v0.16**) | `DQI_VAL_STALE` |
| Valuation timestamp present but value missing | 🔵 Granular | `EMIR.COMP.VALUATION_TIMESTAMP_MISSING` |
| Valuation reported after termination | 🔵 Granular | `EMIR.TST.VALUATION_AFTER_TERMINATION`, `EMIR.LFC.VALUATION_AFTER_TERMINATION` |

### Collateral / margin state quality (`auth.107` + `auth.109`)

| Standard concept | OpenDQI coverage | Indicator / Check ID |
|---|---|---|
| Outstanding-collateralised trade with no MSR row | 🟢 DQI (v0.15) | `DQI_COL_MISSING_STATE` |
| MSR rows with all 4 margin amounts zero/NULL | 🟢 DQI (v0.15) | `DQI_COL_ALL_ZERO` |
| Stale collateral state (older than threshold) | 🟢 DQI (v0.15, **TARGET2 business days since v0.16**) | `DQI_COL_STALE_STATE` |
| Cleared trades specifically missing VM | 🟢 DQI (v0.16) | `DQI_VM_MISSING_FOR_CLEARED` |

### Identification field completeness (`auth.107` / `auth.030`)

| Standard concept | OpenDQI coverage | Indicator ID |
|---|---|---|
| LEI presence rate (RC/OC/CCP/ERR) | 🟢 DQI (v0.16) | `DQI_LEI_MISSING` |
| Entity Responsible for Reporting (ERR) presence | 🟢 DQI (v0.16) | `DQI_ERR_MISSING` |
| Counterparty nature (FC / NFC / NFC+) presence | 🟢 DQI (v0.16) | `DQI_NATURE_MISSING` |
| Corporate sector presence | 🟢 DQI (v0.16) | `DQI_SECTOR_MISSING` |
| LEI format validation (ISO 17442 check digit) | 🔵 Granular | `EMIR.VLD.LEI_FORMAT_{RC,OC,CCP,ERR}` |

### Timeliness (`auth.030` TAR)

| Standard concept | OpenDQI coverage | Indicator / Check ID |
|---|---|---|
| Reporting delay > threshold (T+1 default) | 🟢 DQI (v0.15) | `DQI_TIM_REPORTING_LATE` |
| Per-record late reporting flag | 🔵 Granular | `EMIR.TIM.LATE_REPORTING` |
| Valuation reported after reporting timestamp | 🔵 Granular | `EMIR.TIM.VALUATION_AFTER_REPORTING` |

### Rejections (`auth.092` feedback)

| Standard concept | OpenDQI coverage | Indicator / Check ID |
|---|---|---|
| Rejection rate | 🟢 DQI (v0.15) | `DQI_REJ_RATE` |
| Repeat-rejected UTI detection | 🟢 DQI (v0.15) | `DQI_REJ_REPEAT_UTI` |
| Per-UTI rejection trace | 🔵 Granular | `EMIR.FBK.TR_REJECTED_UTI`, `…TR_MISSING_BUT_NOT_SENT`, `…TR_MISSING_DESPITE_SUBMISSION`, `…TR_INACCURATE_REPORTED` |
| Likely rejection pattern (pre-submission) | 🔵 Granular | `EMIR.PSC.LIKELY_REJECTION_PATTERN`, `…REPEATED_REJECTION` |

### Anomalies / accuracy (`auth.107` TSR + `auth.030` TAR)

| Standard concept | OpenDQI coverage | Indicator / Check ID |
|---|---|---|
| Multi-field anomaly aggregator (negative/zero/abnormal magnitude) | 🟢 DQI (v0.16) | `DQI_ANOMALY_RATE` |
| Negative notional / margin per-record | 🔵 Granular | `EMIR.ACC.NEGATIVE_NOTIONAL`, `…NEGATIVE_INITIAL_MARGIN_POSTED`, etc. (8 negative-* checks) |
| Abnormal magnitude (statistical outliers) | 🔵 Granular | `EMIR.ACC.NOTIONAL_ABNORMAL_MAGNITUDE`, `…ABNORMAL_MATURITY` |
| Zero notional flag | 🔵 Granular | `EMIR.ACC.ZERO_NOTIONAL` |

### Confirmation (TR-side — gated on field presence)

| Standard concept | OpenDQI coverage | Indicator ID |
|---|---|---|
| Confirmation timestamp presence | 🟢 DQI (v0.15, **gated**) | `DQI_CONF_MISSING` |
| Late confirmation per record | 🔵 Granular | `EMIR.RMT.LATE_CONFIRMATION`, `…UNCLEARED_NEEDS_CONFIRMATION` |

### Uniqueness (`auth.107` / `auth.030`)

| Standard concept | OpenDQI coverage | Indicator / Check ID |
|---|---|---|
| Duplicate-UTI / duplicate-report rate | 🟢 DQI (v0.16) | `DQI_DUPLICATE_REPORTS` |
| Duplicate UTI per record | 🔵 Granular | `EMIR.UNI.DUPLICATE_UTI`, `EMIR.TST.DUPLICATE_ACTIVE_UTI` |
| Duplicate NEWT in same batch | 🔵 Granular | `EMIR.TRA.DUPLICATE_NEWT_IN_BATCH`, `EMIR.LFC.DUPLICATE_NEWT_FOR_UTI` |

### Maturity / lifecycle dates

| Standard concept | OpenDQI coverage | Check ID(s) |
|---|---|---|
| Maturity in past / placeholder dates / abnormal maturity | 🔵 Granular | `EMIR.ACC.ABNORMAL_MATURITY`, `EMIR.TST.PLACEHOLDER_MATURITY`, `EMIR.CON.MATURITY_IN_PAST`, `…EFFECTIVE_AFTER_MATURITY` |
| Past-maturity outstanding trades | 🔵 Granular | `EMIR.TST.ACTIVE_PAST_MATURITY` |

### Out-of-scope EMIR concepts (intentional)

| Concept | Rationale |
|---|---|
| Article 11 IM cadence (per-CP IM frequency) | Deferred — requires multi-batch lifecycle tracking ; v0.17+ candidate |
| Dispute resolution timeline | Deferred — requires dispute-specific TR message |
| Compression chain quality (multi-trade) | Granular `EMIR.RMT.COMPRESSION_EVENT_INCOMPLETE` covers single-event ; chain-level is v0.17+ |
| Per-CP detailed reconciliation breakdown table | The DQI rate is shipped ; per-CP drill-down lives in the granular `EMIR.REC.*` issues |

---

## SFTR mapping (v0.16+)

**Layer model** : SFTR reports are structured around 3
logical strata, all carried inline within the same auth.*
message (no separate auth.* per stratum, unlike EMIR's
auth.107 / auth.108 / auth.109 split) :

- **T1** — counterparty identification (RC, OC, ERR, …)
- **T2** — transaction state (loan value, collateral value,
  maturity, settlement, …) — `auth.052` activity & `auth.079`
  state
- **T3** — margining state (IM/VM posted/received,
  pre/post-haircut) — **logically separate** from T2 but
  **inline** in `auth.079`, not shipped as a separate
  auth.* message (functional equivalent of EMIR
  `auth.108`/`auth.109`)

### SFTR DQI coverage (v0.16 — 12 indicators)

| Concept | Layer | Indicator ID |
|---|---|---|
| Loan value missing on outstanding SFT | T2 (`auth.079`) | `DQI_LOAN_VALUE_MISSING_SFTR` |
| Loan value stale (TARGET2) | T2 | `DQI_LOAN_VALUE_STALE_SFTR` |
| Collateral value missing on collateralised SFT | T2 | `DQI_COLLATERAL_VALUE_MISSING_SFTR` |
| Reporting delay > threshold (TAR) | T2 (`auth.052`) | `DQI_TIM_REPORTING_LATE_SFTR` |
| Margin amounts missing on margined SFT | T3 (`auth.079` inline) | `DQI_T3_MARGIN_MISSING` |
| IM posted vs received consistency | T3 | `DQI_T3_MARGIN_CONSISTENCY` |
| Stale T3 margin snapshot (TARGET2) | T3 | `DQI_T3_MARGIN_STALE` |
| Haircut outside plausible range | T2 | `DQI_HAIRCUT_ANOMALY` |
| LEI presence rate (mirror EMIR) | T1 | `DQI_LEI_MISSING_SFTR` |
| Reuse indicator without portfolio code | T2 | `DQI_REUSE_UNTRACKED` |
| Rejection rate **(gated)** — `auth.080` is a reconciliation status, NOT a rejection feedback ; this DQI returns `not_applicable` unless a real rejection feedback is provided | Feedback | `DQI_REJ_RATE_SFTR` |
| Unreconciled status rate from `auth.080` | TSR + recon advice | `DQI_REC_STATUS_UNRECONCILED_SFTR` |

### Out-of-scope SFTR concepts (v0.16)

| Concept | Rationale |
|---|---|
| Margin lending (MGLD) specific DQI rollups | Granular `SFTR.MAR.MGLD_*` + `SFTR.MSR.MGLD_*` cover per-record ; DQI rollup is v0.17 if user demand emerges |
| Trade-by-trade SFTR pairing rate from a recon stats stream | SFTR does not have an auth.091 equivalent ; `auth.080` is per-trade advice, not aggregated stats |
| SFTR auth.083 (missing collateral request) drill-down DQI | Granular `SFTR.MCR.*` family covers it ; rollup is v0.17 candidate |

---

## Methodology — how each indicator was derived

Each indicator in this catalogue was validated against the
8-step protocol :

1. **Identify the business indicator** (in ESMA-public terms)
2. **Extract the required input tables** (`auth.*` messages /
   canonical records)
3. **Extract numerator and denominator** (counted rows /
   eligible population)
4. **Extract joins and grouping keys** (UTI / portfolio code /
   counterparty pair)
5. **Extract thresholds** (default amber / red, configurable
   per-indicator via `dqi:` YAML block)
6. **Extract evidence granularity** (top-N offending UTIs per
   indicator, ≤ 20)
7. **Decide where it belongs** (existing DQI / new DQI /
   existing granular check / documentation only)
8. **Implement in Rust only if** the logic is stable across
   regimes / firms, generalisable beyond a single dataset,
   and testable on synthetic fixtures

Indicators that fail step 8 (e.g. requiring undisclosed
reference data, or specific to a single firm's reporting
template) are tagged "Out of scope" above with the rationale.

## References (public)

- ESMA Q&As on EMIR Refit Reporting (`ESMA74-362-2351`)
- ESMA Q&As on SFTR Reporting (`ESMA80-187-881`)
- ISO 20022 EMIR REFIT usage guidelines :
  - `auth.030.001.04_ESMAUG_DATTAR_1.2.0` (TAR)
  - `auth.107.001.01_ESMAUG_DATSTAT_1.1.0` (TSR)
  - `auth.108.001.01_ESMAUG_DATMDA_1.1.0` (MAR)
  - `auth.109.001.01_ESMAUG_DATMDS_1.1.0` (MSR)
  - `auth.091.001.01_ESMAUG_DATRCS_1.0.0` (Reconciliation Statistics)
  - `auth.092.001.01_ESMAUG_DATTFB_1.0.0` (Trade Reports Validation Feedback)
- ISO 20022 SFTR usage guidelines :
  - `auth.052` (TAR)
  - `auth.079` (TSR including inline T1 / T2 / T3)
  - `auth.080` (Reconciliation status advice)
  - `auth.083` (Missing collateral request)
- TARGET2 holiday calendar : https://www.ecb.europa.eu/paym/target/target2/ (used by `DQI_VAL_STALE`, `DQI_COL_STALE_STATE`, `DQI_LOAN_VALUE_STALE_SFTR`, `DQI_T3_MARGIN_STALE`)
