# auth.* message catalog (EMIR + SFTR)

This document tracks which ISO 20022 `auth.*` messages OpenDQI parses,
the direction of each flow, and how confident we are in our coverage
of the official schema.

We never redistribute SWIFT-licensed XSDs. The descriptions below
characterise our parsers — what fields we recognise and what
namespaces we accept — not the schemas themselves. When the real
schema differs from our synthetic / placeholder structure, the
parsers' leaf tables are designed to be edited in one place.

## Coverage levels

- **verified** — we parse a structure that matches the public ISO 20022
  catalog conventions for this message, and the parser is used in
  production by an OpenDQI CLI command with synthetic fixtures.
- **schema-verified (subset)** — the parser's element paths have been
  aligned with the **real ESMA usage-guideline XSD** (read locally,
  never redistributed) and verified by tests against a schema-shaped
  fixture. Only the documented field subset OpenDQI consumes is
  extracted; full XSD instance validity (every mandatory branch of the
  message tree) is explicitly out of scope and the limits are written
  down in the per-message note.
- **partial** — we parse a plausible structure but have not yet
  validated against the official XSD. The leaf table is documented and
  intended to be adapted when the firm has access to the real schema.
- **placeholder** — we parse a synthetic structure that **diverges
  semantically** from the official message. Useful as a stand-in for
  matching-style or testing flows, but not authoritative.
- **not yet** — on the roadmap, not implemented.

## EMIR

| auth id | ISO/ESMA name (best-effort) | Direction | Coverage | Parser / command |
|---|---|---|---|---|
| `auth.030.001.03` | Derivatives Trade Report (TAR) | firm → TR (and TR → firm replays) | verified | Dual-use: `opendqi emir scan` for firm submissions, `opendqi emir tr-activity-scan` for TR replays. Same adapter `crates/opendqi-xml/src/emir/iso20022.rs`. |
| `auth.092.001.04` | Derivatives Trade Rejection Statistical Report (DATREJ) | TR → firm | **schema-verified (subset)** | `opendqi emir feedback` via `crates/opendqi-xml/src/feedback.rs`, aligned with `auth.092.001.04_ESMAUG_DATREJ_1.0.0`. Lossy projection onto the scalar `FeedbackRecord` (first validation rule only; `Missing`/`Inaccurate` unreachable). Extracted-subset map, limits and the naming caveat in [`auth-messages/emir-auth092.md`](auth-messages/emir-auth092.md). |
| `auth.107.001.01` | Derivatives Trade State Report (TSR) | TR → firm | **schema-verified (subset)** | `opendqi emir tr-state-scan` via `crates/opendqi-xml/src/tr_state.rs`, aligned with `auth.107.001.01_ESMAUG_DATTSR_1.1.0`. Extracted-subset map, ignored branches and verification procedure in [`auth-messages/emir-auth107.md`](auth-messages/emir-auth107.md). Checks: [`tr-state-checks.md`](tr-state-checks.md). |
| `auth.108.001.01` | Derivatives Trade Margin Data Report (MAR) | TR → firm | **schema-verified (subset)** | `opendqi emir mar-scan` via `crates/opendqi-xml/src/emir_mar.rs`, aligned with `auth.108.001.01_ESMAUG_DATMDA_1.1.0`; 8 `EMIR.MAR.*` checks. Extracted-subset map, ignored branches and verification in [`auth-messages/emir-auth108.md`](auth-messages/emir-auth108.md). Checks: [`emir-mar-msr.md`](emir-mar-msr.md). |
| `auth.109.001.01` | Derivatives Trade Margin Data Transaction State Report (MSR) | TR → firm | **schema-verified (subset)** | `opendqi emir msr-scan` via `crates/opendqi-xml/src/emir_msr.rs`, aligned with `auth.109.001.01_ESMAUG_DATMDS_1.1.0`; 8 `EMIR.MSR.*` checks. Extracted-subset map, ignored branches and limits in [`auth-messages/emir-auth109.md`](auth-messages/emir-auth109.md). Checks: [`emir-mar-msr.md`](emir-mar-msr.md). |
| `auth.091.001.02` | Derivatives Trade Reconciliation Statistical Report | TR → firm | **schema-verified (subset)** | `opendqi emir recon-stats` via `crates/opendqi-xml/src/emir_recon_stats.rs`, aligned with `auth.091.001.02_ESMAUG_DATREC_1.0.0`; 4 `EMIR.RST.*` checks. Rates are **derived** by accumulating cohort `TtlNbOfTxs` by `Pairg`/`Rcncltn` (no explicit rate fields); `outstanding_*` has no source → `OUTSTANDING_UNPAIRED_HIGH` unreachable. The per-transaction `TxDtls/RcncltnRpt`/`MtchgCrit` detail is now also projected onto `ReconciliationRecord` (cohort-inherited status; `Val1`≠`Val2` mismatch) and `recon-stats` folds the resulting `EMIR.REC.*` issues into `recon_stats_issues.csv`. Map, derivation and limits in [`auth-messages/emir-auth091.md`](auth-messages/emir-auth091.md). Checks: [`emir-recon-stats.md`](emir-recon-stats.md). |
| `auth.106.001.01` | Derivatives Trade Warnings Report (DATWRN) | TR → firm | **schema-verified (subset)** | `opendqi emir warnings` via `crates/opendqi-xml/src/emir_warnings.rs`, aligned with `auth.106.001.01_ESMAUG_DATWRN_1.1.0`; 5 `EMIR.WRN.*` checks. Report-level missing/outdated valuation & margin and abnormal-values rates are **derived** from the counts; the per-counterparty `Wrnngs` breakdown is a deferred subset. Map, derivation and limits in [`auth-messages/emir-auth106.md`](auth-messages/emir-auth106.md). Checks: [`emir-warnings.md`](emir-warnings.md). **Note:** a legacy *synthetic* "pairing" path under this namespace (the old `opendqi emir reconcile`) still exists and is removed in the next increment — see "Naming caveat" below. |

## SFTR

| auth id | ISO/ESMA name (best-effort) | Direction | Coverage | Parser / command |
|---|---|---|---|---|
| `auth.052.001.02` | SFT Trade Report | firm → TR | verified | `opendqi sftr scan` via `crates/opendqi-xml/src/sftr/iso20022.rs`. Also re-used by `opendqi sftr tr-activity-scan` for TR replays. |
| `auth.080.001.02` | SFT Reconciliation Status Advice | TR → firm | **schema-verified (subset)** | `opendqi sftr reconcile` via `crates/opendqi-xml/src/reconciliation.rs`, aligned with `auth.080.001.02_ESMAUG_SFTREC_1.1.0` (re-homed from `sftr feedback` — it is reconciliation, not feedback). Derive map onto `ReconciliationRecord`, ignored branches and documented limits in [`auth-messages/sftr-auth080.md`](auth-messages/sftr-auth080.md). |
| `auth.079.001.02` | Securities Financing Transaction State Report (SFTR TSR) | TR → firm | **schema-verified (subset)** | `opendqi sftr tr-state-scan` via `crates/opendqi-xml/src/sftr_tr_state.rs`, aligned with `auth.079.001.02_ESMAUG_SFTTRS_1.1.0`; 6 `SFTR.TST.*` + 6 `SFTR.MSR.MGLD_*` checks. Extracted-subset map, the 4-way loan choice, ignored branches and documented limits in [`auth-messages/sftr-auth079.md`](auth-messages/sftr-auth079.md). |
| `auth.083.001.NN` | (SFTR analog of `auth.106`) | TR → firm | **placeholder (matching-style)** | Same caveat as `auth.106`. |

## Naming caveat — `auth.106` and `auth.083`

The current `opendqi {emir,sftr} reconcile` subcommands parse a
synthetic pairing / matching structure with `<Rcncltn>` blocks
carrying `PrngSts` (`PAIRED` / `UNPAIRED`), `RcncltnSts` (`RECONCILED`
/ `UNRECONCILED`), and a repeating `MismatchedField` list. The 6
`*.REC.*` checks (`UNPAIRED_TRADE`, `UNRECONCILED_TRADE`,
`FIELD_MISMATCH`) operate on that shape.

This shape is **plausible for a counterparty-pairing report from a
TR** but is not the documented semantic of the official `auth.106` /
`auth.083`, which in the ESMA catalog appear to carry **data-quality
warnings**.

Concretely:

- The parser code in `crates/opendqi-xml/src/reconciliation.rs` and
  the `*.REC.*` checks remain useful for firms that have a
  matching-style file from their TR.
- When we have access to a real `auth.106` XSD, the parser will
  either be extended (if the message is a superset) or renamed to
  `tr-warnings` and a new `auth.106` parser will be added in its
  place. No fixture or check ID is locked to the current shape.
- The roadmap places the resolution of this caveat at Phase 3
  ("Rejection analytics"), where `auth.092` and `auth.106` will be
  consolidated under a single rejection / warning intelligence layer.

This caveat is mirrored in [`reconciliation-checks.md`](reconciliation-checks.md)
and [`tr-reconciliation.md`](tr-reconciliation.md).

## Naming caveat — `auth.092` and `auth.080` ("feedback")

OpenDQI's `opendqi emir feedback` workflow models a per-UTI
`FeedbackRecord` with a single `reason_code` and a
`FeedbackType ∈ {Rejected, Missing, Inaccurate, ReconciliationBreak}`.
Against the **real** schemas this is a deliberate, documented
projection — the messages are not per-UTI feeds:

- **`auth.092.001.04`** is the *Derivatives Trade Rejection Statistical
  Report* (DATREJ): per-counterparty aggregate counts plus, per
  rejected transaction, a **repeating** `DtldVldtnRule` validation-rule
  list and a `Sts` ∈ `{ACPT, RJCT, INCF, CRPT, NAUT}`. There is **no
  "Missing" or "Inaccurate" branch**. OpenDQI's EMIR path now parses
  this real envelope but projects each rejected transaction onto the
  scalar model (first validation rule only; `ACPT` skipped). So of the
  four `EMIR.FBK.*` checks only `TR_REJECTED_UTI` is reachable from
  real `auth.092`; the two `*_MISSING_*` and `*_INACCURATE_*` checks
  are unreachable from this message. Full detail:
  [`auth-messages/emir-auth092.md`](auth-messages/emir-auth092.md).
- **`auth.080.001.02`** is the SFTR *Securities Financing Reporting
  Reconciliation Status Advice* — a pairing/reconciliation state
  machine with field-level matching criteria, **not** rejection
  feedback. It has been **re-homed**: the real `auth.080.001.02`
  parser lives in `reconciliation.rs` and is reached via
  `opendqi sftr reconcile`, mapping onto `ReconciliationRecord`
  (`schema-verified (subset)` — see
  [`auth-messages/sftr-auth080.md`](auth-messages/sftr-auth080.md)).
  Consequently **SFTR has no rejection-feedback message**: the
  synthetic `opendqi sftr feedback` command, its `auth.080.001.01`
  parser and the four `SFTR.FBK.*` checks were **removed** in
  Milestone 0.4. SFTR feedback no longer exists; `opendqi sftr
  tr-audit` is TAR+TSR-only (it has no
  `SFTR.AUD.REJECTED_BUT_OUTSTANDING_IN_TSR` — that check is EMIR-only).

The faithful re-model milestone is **complete**: the `auth.092`
validation-rule list, the real `auth.080` parser, the EMIR `auth.091`
per-transaction `RcncltnRpt`/`MtchgCrit` detail, and the removal of
the synthetic SFTR-feedback command/parser/checks are all shipped.

## Adding a new auth.* message

1. Add a row to the table above with `not yet` coverage and the
   intended `Phase N` label.
2. When implementing, decide between a shared adapter (e.g.
   `reconciliation.rs` namespace-dispatches `auth.083`/`auth.080`) or
   a dedicated module (e.g. `iso20022.rs` per regime).
3. Document the synthetic namespace used by the fixture and flag the
   coverage level honestly. Move to `verified` only when the parser
   has been confronted with the official XSD (even indirectly via a
   firm-provided real file).
