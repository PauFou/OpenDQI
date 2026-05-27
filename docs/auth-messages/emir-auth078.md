# EMIR `auth.078` — Non-existent in the ESMA bundle

Per-message coverage note. Parent catalog:
[`../auth-messages.md`](../auth-messages.md).

## Status: NOT IMPLEMENTED — message does not exist

`auth.078` does **not** appear in the ESMA-published ISO 20022
message bundle for EMIR. There is no XSD to parse, no envelope
to model, no DQ surface to ship. This page exists so that
adopters who notice the gap in our 10-out-of-11 EMIR coverage
table understand exactly why the 11th message is skipped: it is
not skipped, it does not exist.

## How we discovered this (v0.18 plan D)

The original v0.18 release plan reserved Phase D for an
SFTR-side `auth.078 SecuritiesFinancingReportingPairingRequest`
message — the conjectured name suggested a pairing-request
envelope for SFTR. When the implementation team downloaded the
official ESMA SFTR bundle and verified its XSD index against
the conjectured message name, the result was that `auth.078`
is not in the published set. Pairing semantics on the SFTR
side are carried by `auth.080`
(`SecuritiesFinancingReportingReconciliationStatusAdviceV02`),
which OpenDQI has shipped since v0.16 (`MissingCollateral`
subcommand + cross-message reconciliation workflows).

The v0.18 plan was pivoted on the spot from `auth.078` to
`auth.084`
(`SecuritiesFinancingReportingTransactionStatusAdviceV02`,
SFTR rejection feedback — a real, published, DQ-actionable
message that was missing from our coverage), which shipped in
v0.18 Phase D. That commitment to "what actually exists" is
documented in `crates/opendqi-core/src/model.rs` near the
`SftrReuseStateRecord` struct.

## Where the EMIR side stands

The EMIR `auth.078` placeholder some catalogues still list is a
ghost reference. The EMIR ISO 20022 set we cover is:

- `auth.029` — Trade Report Query (firm → TR, envelope-only; v0.20)
- `auth.030` — Trade Report (firm → TR, the TAR)
- `auth.031` — Status Advice (TR → firm ack; v0.20 envelope-only)
- `auth.090` — Position Set Report (TR aggregates)
- `auth.091` — Reconciliation Statistical Report
- `auth.092` — Trade Rejection Statistical Report (the feedback workflow)
- `auth.106` — Trade Warnings Report
- `auth.107` — Trade State Report (TR snapshot, the TSR)
- `auth.108` — Margin Activity Report
- `auth.109` — Margin State Report

That is the complete EMIR bundle as published. v0.20 brings us to
**10 of 10 actually-existing EMIR messages parsed** (auth.078
counted in the "11" tally was always vapour).

## If your bundle says otherwise

If a downstream catalogue, training material, or third-party tool
references `auth.078` as an EMIR message, please:

1. Verify against the [ESMA ISO 20022 reporting](https://www.esma.europa.eu/document-types/iso-20022)
   page directly.
2. Open an issue on this repo with the link — if `auth.078` is
   ever published, OpenDQI will add support following the v0.18
   parser pattern (under 800 lines, ~5 commits per the v0.20
   precedent).

Until then, this gap is honest: there is nothing to parse, so
we don't pretend to parse it.
