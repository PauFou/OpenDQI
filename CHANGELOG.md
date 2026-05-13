# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial Cargo workspace with crates: `opendqi-core`, `opendqi-io`, `opendqi-report`, `opendqi-cli`, plus `opendqi-xml` and `opendqi-server` stubs.
- EMIR canonical record model and shared DQ issue / dimension / severity types.
- CSV ingestion with YAML field mapping.
- Five MVP EMIR checks: missing UTI, missing valuation, abnormal maturity, duplicate UTI, late reporting.
- `opendqi emir scan` CLI subcommand producing `summary.json`, `issues.csv`, and `report.html`.
- Synthetic example fixture under `examples/emir/`.
- EMIR XML ingestion using the simplified OpenDQI XML format v0.1 (`docs/xml-format.md`). `opendqi emir scan` now accepts `.xml` files and directories of mixed `.csv`/`.xml` inputs.
- Streaming XML well-formedness check via `quick-xml` and three format-level issue types: `EMIR.FMT.XML_NOT_WELLFORMED` (critical), `EMIR.FMT.XML_UNSUPPORTED_NAMESPACE` (warning), `EMIR.FMT.XML_UNKNOWN_ELEMENT` (info, de-duplicated per file).
- Fixtures `examples/emir/sample.xml` and `examples/emir/broken/malformed.xml`.

### Changed

- `--mapping` is now optional on `opendqi emir scan`; it is only required when the input set contains at least one CSV file.
- `discover_inputs` was renamed to `discover_emir_inputs` and accepts both `.csv` and `.xml` files.
- `opendqi emir scan` accepts an optional `--xsd <path>` flag. When set, every XML input is also validated against the schema and violations are surfaced as `EMIR.FMT.XSD_VIOLATION` issues (severity `high`). A dedicated `xsd_errors.csv` is written alongside the standard reports.
- `opendqi emir validate <input> --xsd <path>` is now a real command: it performs a streaming well-formedness check, runs XSD validation via `xmllint`, prints `path:line: message` to stderr for each violation, and exits non-zero when any issue is found.
- New `XsdValidator` trait in `opendqi-xml` with `ExternalXmllintValidator` (shells out to `xmllint --noout --schema`) and `NoopValidator` implementations.
- Canonical XSD `examples/emir/schemas/opendqi-emir-v0.1.xsd` and violation fixture `examples/emir/violates-schema.xml`.
- Documentation: new `docs/xsd-validation.md`.
- **ISO 20022 `auth.030.001.03` (EMIR Refit) ingestion adapter**. `opendqi emir scan` auto-detects the namespace and routes to the new extractor — no flag required. All 8 EMIR action types are recognised (`NEWT`/`MODI`/`CORR`/`ETRM`/`POSC`/`VALU`/`MARU`/`OTHR`); both legs of swap-like products are captured.
- `EmirRecord` extended with leg-2 notional, margin posted/collected pairs, clearing CCP LEI, master agreement metadata, intragroup / hedging indicators, valuation type, option greeks, and a `raw_fields: BTreeMap<String, String>` catch-all for any auth.030 leaf not in the typed-routing table.
- Synthetic fixture `examples/emir/iso20022/sample.xml` (10 trades, all 8 action types, hand-authored — not derived from ESMA/SWIFT examples).
- Documentation: new `docs/iso20022-emir.md` (mapping table + how to obtain the official SWIFT-licensed XSD).
- **16 new EMIR data-quality checks driven by the ESMA EMIR Refit Validation Rules**, bringing the catalog to 21 checks total. New checks: counterparty / entity LEI shape (3 checks), ISO 4217 currency shape (2 checks), counterparty / currency / valuation completeness (5 checks), zero / negative notional (2 checks), valuation-after-reporting timeliness, reporting-before-execution / cleared-requires-CCP / valuation-after-termination consistency. See [`docs/emir-checks.md`](docs/emir-checks.md) for the full catalog with ESMA-VR references.
- Synthetic fixture `examples/emir/extended-checks.csv` + `extended-checks.yml` exercising one positive case per new check.
- `opendqi-core/src/dq/formats.rs`: shared ISO 17442 LEI shape and ISO 4217 currency shape validators.
- `opendqi-io/src/csv_in.rs`: now ingests the `clearing_ccp_lei` column when present in the mapping.

### Changed

- Existing fixtures (`examples/emir/sample.csv`, `sample.xml`, `iso20022/sample.xml`) intentionally exhibit LEI shapes that end with `AA` and valuation timestamps later than reporting timestamps; the new checks correctly flag these as defects. Issue counts for those fixtures rise from 7/7/8 to ~38/38/47. This is expected and demonstrates the new coverage in action — the fixtures are not yet rewritten to be defect-free.

### Added (SFTR)

- **SFTR regime support**: new canonical `SftrRecord` struct, `SftrCheck` trait, `default_sftr_checks()` registry and `run_all_sftr` runner in `opendqi-core`.
- **ISO 20022 `auth.052.001.02` ingestion adapter** (`opendqi-xml::sftr`): namespace-aware routing, selective extractor with the same `raw_fields` catch-all pattern as the EMIR adapter. Recognises all SFTR action types (NEWT/MODI/CORR/ETRM/VALU/COLU/MARU/REUU/POSC/OTHR) and the four SFT wrappers (Repo/BSB/SLEB/MGLD).
- **5 MVP SFTR data-quality checks**: `SFTR.COMP.UTI_MISSING`, `SFTR.COMP.COLLATERAL_VALUE_MISSING`, `SFTR.COMP.HAIRCUT_MISSING`, `SFTR.TIM.LATE_REPORTING`, `SFTR.UNI.DUPLICATE_UTI`. See [`docs/sftr-checks.md`](docs/sftr-checks.md).
- **CLI**: `opendqi sftr scan <path> --out <dir>` is now a real command (previously a placeholder). XML-only ingestion for this milestone.
- Synthetic fixture `examples/sftr/iso20022/sample.xml` (10 SFTs hand-authored covering Repo / BSB / SLEB and all DQ patterns) plus integration test `crates/opendqi-xml/tests/sftr_integration.rs`.
- Documentation: new `docs/sftr-checks.md` and `docs/iso20022-sftr.md`.

### Added (SFTR parity with EMIR)

- **15 additional SFTR data-quality checks**, bringing the SFTR catalog from 5 to **20** (same dimension coverage as EMIR). Highlights: LEI shape on the three party LEIs, ISO 4217 shape on loan / collateral currencies, ISO 6166 shape on collateral ISIN, missing-counterparty checks, missing-currency checks, negative loan / collateral, haircut out-of-range, settlement-before-execution, maturity-before-effective.
- **`opendqi sftr scan --xsd <xsd>`** is now active: schema violations surface as `SFTR.FMT.XSD_VIOLATION` (high) issues, a dedicated `xsd_errors.csv` is written alongside the standard reports.
- **`opendqi sftr validate <input> --xsd <xsd>`** is a real command (well-formedness + XSD validation, exits non-zero on any defect).
- New ISIN shape validator `is_valid_isin` in `opendqi-core/src/dq/formats.rs`.
- Extended SFTR fixture `examples/sftr/iso20022/extended.xml` exercising one positive case per new check.

### Added (EMIR margin / clearing / enums / dates depth)

- **30 additional EMIR data-quality checks**, bringing the EMIR catalog from 21 to **51** (covers margin amounts, clearing metadata, enumerated codes, and a broader set of date / consistency rules).
  - Margin (8): negative initial/variation margin posted/collected, `MARU` action requires margin, FLCL requires initial/variation margin posted + collateral portfolio code.
  - Clearing (5): CCP LEI shape, NCLR forbids CCP, clearing status missing/enum, intragroup indicator missing.
  - Enumerations (8): `action_type` / `event_type` / `valuation_type` / `trading_capacity` / `asset_class` / `nature` / `master_agreement_type` / `collateralisation_category`.
  - Dates & consistency (9): event-before-execution, maturity-in-past, termination/effective after maturity, ETRM requires termination date, and four "missing" completeness checks (nature / trading capacity / master agreement / asset class).
- New `is_in` helper in `opendqi-core/src/dq/formats.rs` for case-insensitive small-enum matching.
- `opendqi-io/src/csv_in.rs` now ingests `nature`, `corporate_sector`, `trading_capacity`, `valuation_type`, `master_agreement_type`, `master_agreement_version`, `intragroup_indicator` columns when present in the mapping.
- Synthetic fixture `examples/emir/margin-and-enums.csv` + `margin-and-enums.yml`: 31 rows exercising each of the 30 new check_ids at least once.

### Changed (EMIR depth side effects)

- The pre-existing `examples/emir/extended-checks.csv` now produces ~87 issues (up from 17) because the new completeness/enum checks correctly flag many fields that fixture leaves unpopulated. This is expected — the fixture is preserved as-is so the additional coverage is visible.

### Added (TR reconciliation ingestion + 6 `*.REC.*` checks)

- New canonical type `ReconciliationRecord` in `opendqi-core::model`. Captures UTI, both counterparty LEIs, pairing status, reconciliation status, list of mismatched fields, and timestamp.
- New `ReconciliationCheck` / `SftrReconciliationCheck` traits, parallel to the existing `Check` / `FeedbackCheck` / `LifecycleCheck` traits. Pure function of `(records, prior, ctx)`.
- **6 TR reconciliation checks** (3 EMIR + 3 SFTR): bringing the catalog to **129 single-batch + 8 lifecycle + 8 feedback + 6 reconciliation = 151 checks**.
  - `EMIR.REC.UNPAIRED_TRADE` / `SFTR.REC.UNPAIRED_TRADE` — TR reports the trade as UNPAIRED (counterparty has not submitted) (Consistency / High).
  - `EMIR.REC.UNRECONCILED_TRADE` / `SFTR.REC.UNRECONCILED_TRADE` — TR reports the trade as UNRECONCILED — paired but fields disagree (Consistency / High).
  - `EMIR.REC.FIELD_MISMATCH` / `SFTR.REC.FIELD_MISMATCH` — one issue per field name in the TR's `<MismatchedField>` list (Accuracy / High).
- New `crates/opendqi-xml/src/reconciliation.rs`: streaming `NsReader` adapter for ISO 20022 `auth.106` (EMIR) and `auth.083` (SFTR) reconciliation messages. Recognises `<Rcncltn>` blocks with `UnqTxIdr` / `RptgCtrPty/LEI` / `OthrCtrPty/LEI` / `PrngSts` / `RcncltnSts` / repeating `<MismatchedField>` leaves, plus a header `RcncltnDtTm` timestamp. SWIFT-licensed XSDs are not redistributed; the adapter parses a plausible structure aligned with the public ISO 20022 catalog.
- New `reconciliations` table in the SQLite history store (additive schema). New `Store::persist_reconciliation_batch` method. Mismatched-field lists serialised as JSON.
- New CLI subcommands `opendqi emir reconcile <input> --store <PATH> --out <DIR>` and `opendqi sftr reconcile <input> --store <PATH> --out <DIR>`. `--store` is required.
- Synthetic fixtures `examples/emir/reconciliation/auth106-sample.xml` and `examples/sftr/reconciliation/auth083-sample.xml`, each exercising the 3 check_ids of its regime.
- Documentation: new `docs/reconciliation-checks.md` (catalog) and `docs/tr-reconciliation.md` (usage, XML structure, legal note).

### Added (Store v2 — feedbacks table + Open/Resolved/Stale workflow)

- New `feedbacks` table in the SQLite history store, additive to v1 schema. Persists every TR feedback row with a `status` column (`open` / `resolved` / `stale`), `status_set_at` timestamp, and indexes on `(uti, status)` and `(status)`.
- New `Store` methods: `persist_feedback_batch`, `list_feedbacks(regime, uti, status)`, `update_feedback_status(uti, new_status)`. New public `FeedbackRow` struct.
- `opendqi {emir,sftr} feedback` ingestion now **persists** every parsed feedback record into the store (transparent for the user — the store was already opened). Existing callers pass through unchanged.
- New **top-level** CLI subcommand `opendqi feedback`:
  - `feedback list --store <PATH> [--regime emir|sftr] [--uti <UTI>] [--status open|resolved|stale]`
  - `feedback resolve --store <PATH> --uti <UTI>`
  - `feedback stale --store <PATH> --uti <UTI>`
- Idempotent: re-marking a row to its current status is a no-op.
- Documentation: `docs/history-store.md` (new "feedbacks table" + "workflow" sections), `docs/feedback-checks.md` (persistence note).

### Added (TR feedback ingestion + 8 `*.FBK.*` checks)

- New canonical types `FeedbackType` (`Rejected` / `Missing` / `Inaccurate` / `ReconciliationBreak`) and `FeedbackRecord` in `opendqi-core::model`.
- New `FeedbackCheck` / `SftrFeedbackCheck` traits in `opendqi-core::dq` — pure functions of `(feedback, prior, ctx)`, parallel to the existing `Check` / `SftrCheck` / `LifecycleCheck` traits.
- **8 TR feedback checks** (4 EMIR + 4 SFTR), bringing the catalog to **129 single-batch + lifecycle + 8 feedback = 137 checks** when the history store is enabled:
  - `EMIR.FBK.TR_REJECTED_UTI` / `SFTR.FBK.TR_REJECTED_UTI` — TR rejected the submission (Validity / Critical).
  - `EMIR.FBK.TR_MISSING_BUT_NOT_SENT` / `SFTR.FBK.TR_MISSING_BUT_NOT_SENT` — TR signals missing, no prior NEWT in store, confirmed gap (Completeness / High).
  - `EMIR.FBK.TR_MISSING_DESPITE_SUBMISSION` / `SFTR.FBK.TR_MISSING_DESPITE_SUBMISSION` — TR signals missing but a prior NEWT exists, TR ingestion failure or stale feedback (Consistency / Critical).
  - `EMIR.FBK.TR_INACCURATE_REPORTED` / `SFTR.FBK.TR_INACCURATE_REPORTED` — TR flagged inaccurate field (Accuracy / High).
- New `crates/opendqi-xml/src/feedback.rs`: streaming `NsReader` adapter for ISO 20022 `auth.092` (EMIR) and `auth.080` (SFTR) feedback messages. Detects `<Rjctd>` / `<Mssng>` / `<Inaccrt>` / `<RcncltnBrk>` wrappers, captures `UnqTxIdr` / `RsnCd` / `RsnDesc` / `FldNm` leaves, and propagates the header `FdbckDtTm` to every record. The XSDs are SWIFT-licensed and not redistributed; the adapter parses a plausible structure aligned with the public ISO 20022 catalog.
- New CLI subcommands `opendqi emir feedback <input> --store <PATH> --out <DIR>` and `opendqi sftr feedback <input> --store <PATH> --out <DIR>`. `--store` is required (the cross-reference is the value-add of the flow).
- Synthetic fixtures `examples/emir/feedback/auth092-sample.xml` and `examples/sftr/feedback/auth080-sample.xml`, each exercising all 4 check_ids of its regime end-to-end.
- Documentation: new `docs/feedback-checks.md` (catalog) and `docs/tr-feedback.md` (usage, expected XML structure, legal note).

### Added (History store + cross-batch lifecycle checks)

- New `opendqi-store` crate: SQLite-backed history store (`rusqlite` with `bundled` feature, no system libsqlite dependency). Persists scanned EMIR / SFTR records into `scans` + `emir_records` + `sftr_records` tables, with indexes on `(uti)` and `(uti, action_type)`.
- New `LifecycleCheck` / `SftrLifecycleCheck` traits in `opendqi-core` — pure functions of `(current, prior, ctx)`. The existing 121 single-batch checks are unchanged and the new traits run in parallel, not in place of them.
- **8 cross-batch lifecycle checks** (5 EMIR + 3 SFTR), bringing the OpenDQI catalog to **121 single-batch + 8 lifecycle = 129 checks** when the history store is enabled:
  - `EMIR.LFC.MODI_WITHOUT_NEWT` / `SFTR.LFC.MODI_WITHOUT_NEWT` — modification on a UTI with no prior NEWT in the store (Consistency / High).
  - `EMIR.LFC.ETRM_WITHOUT_NEWT` / `SFTR.LFC.ETRM_WITHOUT_NEWT` — early termination on a UTI with no prior NEWT (Consistency / High).
  - `EMIR.LFC.DUPLICATE_NEWT_FOR_UTI` / `SFTR.LFC.DUPLICATE_NEWT_FOR_UTI` — new trade declared for a UTI that already has a prior NEWT (Uniqueness / Critical).
  - `EMIR.LFC.VALUATION_REGRESSION` — VALU whose `valuation_timestamp` is earlier than the latest known prior VALU for the same UTI (Consistency / Warning).
  - `EMIR.LFC.VALUATION_AFTER_TERMINATION` — VALU whose `valuation_timestamp.date()` is on or after a prior ETRM's `termination_date` for the same UTI (Consistency / High).
- New `--store <PATH>` flag on `opendqi emir scan` and `opendqi sftr scan`. When set: scanned records are persisted to the SQLite file at `PATH`, prior records for the current batch's UTIs are loaded, and the lifecycle checks run on top of the regular single-batch suite. Without the flag, OpenDQI runs entirely in memory and never opens a database — strict zero-regression for existing workflows.
- `.gitignore` now excludes `*.sqlite*`, `*.db*`, and their journal / WAL / SHM companions.
- Documentation: new `docs/lifecycle-checks.md` (catalog) and `docs/history-store.md` (usage, schema, operations).

### Added (SFTR tier 2 — parity with EMIR tier 2)

- **20 additional SFTR data-quality checks**, bringing the SFTR catalog from 20 to **40** (full parity in dimension coverage with the EMIR catalog at this depth).
  - Cross-field consistency (6): `SELF_DEALING`, `LOAN_NEEDS_CURRENCY`, `COLL_NEEDS_CURRENCY`, `LOAN_COLL_CURRENCY_MISMATCH`, `REBATE_REQUIRES_REPO_OR_BSB`, `LENDING_FEE_REQUIRES_SLEB`.
  - Action / event semantics (5): `NEWT_FORBIDS_PRIOR_UTI`, `NEWT_FORBIDS_TERMINATION_DATE`, `ETRM_REQUIRES_TERMINATION_DATE`, `COLU_REQUIRES_PORTFOLIO`, `REUU_REQUIRES_REUSE_INDICATOR`.
  - SFT-type / action enums (3): `SFT_TYPE_MISSING`, `SFT_TYPE_ENUM`, `ACTION_TYPE_ENUM`.
  - Decimal precision (4): `LOAN_PRECISION`, `COLLATERAL_PRECISION`, `HAIRCUT_PRECISION`, `RATE_PRECISION` (rebate / lending-fee).
  - Master agreement (2): `MASTER_AGREEMENT_VERSION_FORMAT`, `GMRA_GMSLA_VERSION_PLAUSIBLE`.
- Synthetic fixture `examples/sftr/iso20022/tier2.xml` exercising 15 of the 20 new check_ids end-to-end. Five checks (`SFT_TYPE_MISSING`, `SFT_TYPE_ENUM`, `ACTION_TYPE_ENUM`, `REBATE_REQUIRES_REPO_OR_BSB`, `LENDING_FEE_REQUIRES_SLEB`) cannot be triggered through the current `auth.052` adapter (which forces `sft_type` and `action_type` from wrappers, and only populates `rebate_rate` / `lending_fee` on canonical XSD paths) and are exercised by unit tests inside each check file.

### Changed (SFTR depth side effects)

- The pre-existing `examples/sftr/iso20022/extended.xml` now produces 18 issues (up from 15). The 3 additional issues come from the new tier-2 consistency checks correctly flagging defects the fixture already contained: `LOAN_NEEDS_CURRENCY` and `COLL_NEEDS_CURRENCY` co-fire with the existing completeness checks on rows 10 / 11, and `LOAN_COLL_CURRENCY_MISMATCH` flags row 6 (loan `EUR`, collateral `EU`). The fixture is preserved as-is so the additional coverage is visible.

### Added (EMIR tier 2 — cross-field / asset-class / precision / action semantics)

- **30 additional EMIR data-quality checks**, bringing the EMIR catalog from 51 to **81**.
  - Cross-field consistency (10): notional/valuation/price currency mismatches, leg-1/leg-2 same currency, self-dealing, price-requires-currency, IM/VM need a portfolio, leg-2 notional needs its currency, hedging indicator requires non-financial nature, MtM change requires valuation.
  - Asset-class specific (7): IR requires notional / leg-1 frequency, FX requires leg-2 currency, EQ / CR require underlying, commodity requires product id, asset class declared without product id.
  - Action / event semantics (6): VALU requires valuation, NEWT forbids prior UTI / termination date, POSC / MARU require portfolio code, ETRM expects a final valuation.
  - Decimal precision (4): notional / valuation / price / margin amounts must fit ESMA `decimal:18.5`.
  - Master agreement (3): version format (4-digit year), version missing when type set, ISDA version in `{1992, 2002, 2017}`.
- `is_in` and `within_decimal_bounds` helpers in `opendqi-core/src/dq/formats.rs`.
- `opendqi-io/src/csv_in.rs` now ingests leg-2 notional/currency, leg-1/leg-2 payment frequency, master-agreement version, mtm value change, hedging indicator, and the option greeks (delta / gamma / vega).
- Synthetic fixture `examples/emir/tier2.csv` + `tier2.yml`: 31 rows exercising each of the 30 tier-2 check_ids at least once.
