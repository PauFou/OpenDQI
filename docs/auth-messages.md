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
| `auth.030.001.03` | Derivatives Trade Report (TAR) | firm → TR (and TR → firm replays) | verified | `opendqi emir scan` via `crates/opendqi-xml/src/emir/iso20022.rs`. Phase 2 will add a dedicated TR-output mode (`tr-activity-scan`). |
| `auth.092.001.NN` | Trade Reports Rejected / Missing / Inaccurate (feedback) | TR → firm | partial | `opendqi emir feedback` via `crates/opendqi-xml/src/feedback.rs`. Phase 3 will deepen analytics (top causes, ageing, rejected → accepted). |
| `auth.107.001.NN` | Trade State Report (TSR) | TR → firm | **verified (synthetic schema)** | `opendqi emir tr-state-scan` via `crates/opendqi-xml/src/tr_state.rs`. Leaf table documented in [`tr-state-checks.md`](tr-state-checks.md); designed to be edited when the real XSD is available. |
| `auth.108.001.NN` | Margin Activity | firm → TR / TR → firm | not yet | Roadmap. |
| `auth.109.001.NN` | Margin State | TR → firm | not yet | Roadmap. |
| `auth.091.001.NN` | Reconciliation Statistics | TR → firm | not yet | Roadmap. |
| `auth.106.001.NN` | Data-quality Warnings (official) | TR → firm | **placeholder (matching-style)** | See "Naming caveat" below. |

## SFTR

| auth id | ISO/ESMA name (best-effort) | Direction | Coverage | Parser / command |
|---|---|---|---|---|
| `auth.052.001.02` | SFT Trade Report | firm → TR | verified | `opendqi sftr scan` via `crates/opendqi-xml/src/sftr/iso20022.rs`. |
| `auth.080.001.NN` | SFT Rejected / Missing / Inaccurate (feedback) | TR → firm | partial | `opendqi sftr feedback` (same shared adapter as `auth.092`). |
| `auth.079.001.NN` | SFT Trade State Report (TSR) | TR → firm | not yet — Phase 6 | Will mirror the EMIR `tr-state-scan` once the EMIR pattern is stable. |
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

## Adding a new auth.* message

1. Add a row to the table above with `not yet` coverage and the
   intended `Phase N` label.
2. When implementing, decide between a shared adapter (e.g.
   `feedback.rs` handles both `auth.092` and `auth.080`) or a
   dedicated module (e.g. `iso20022.rs` per regime).
3. Document the synthetic namespace used by the fixture and flag the
   coverage level honestly. Move to `verified` only when the parser
   has been confronted with the official XSD (even indirectly via a
   firm-provided real file).
