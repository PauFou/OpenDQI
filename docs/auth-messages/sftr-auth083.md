# SFTR `auth.083` — Securities Financing Reporting Missing Collateral Request

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Command: `opendqi sftr missing-collateral`.

## Business meaning

`auth.083` is the trade repository's **operational request, sent to the
firm, to supply the collateral information that is missing for a given
SFT**. It is a flat list of transactions for which the TR has no (or
incomplete) collateral data and expects a follow-up submission. It is
**not** reconciliation (that is the real `auth.080`), **not** rejection
feedback (SFTR has no feedback message), and **not** the "SFTR analogue
of EMIR `auth.106`". OpenDQI turns each listed transaction into an
actionable `SFTR.MCR.*` issue.

## Direction

**TR → firm.**

## Coverage status

**schema-verified (subset).** The real `auth.083.001.02` envelope is
parsed (`crates/opendqi-xml/src/sftr_missing_collateral.rs`,
`read_sftr_missing_collateral_xml`, namespace-dispatched) and projected
onto a new scalar `MissingCollateralRecord` — a deliberate, documented
derive-subset. No existing model/check/store-schema change.

## Real envelope

```
Document
└─ SctiesFincgRptgMssngCollReq  (SecuritiesFinancingReportingMissingCollateralRequestV02)
   └─ TxId (1..unbounded)  (TradeTransactionIdentification18)   ← RECORD
      ├─ RptgCtrPty/LEI                              [1..1]
      ├─ OthrCtrPty (PartyIdentification236Choice, choice):
      │     ├─ Lgl/LEI
      │     └─ Ntrl/Id/Id   (natural-person Max50Text id)
      ├─ UnqTradIdr  (Max52Text)                     [0..1]   (the UTI)
      └─ MstrAgrmt (MasterAgreement7) [0..1]:
            Tp/Tp (MasterAgreementType1Code), Vrsn (ISORestrictedYear,
            1900..2099) [0..1], OthrMstrAgrmtDtls (Max50Text) [0..1]
```

Accepted root namespace:
`urn:iso:std:iso:20022:tech:xsd:auth.083.001.02` (mismatch →
`SFTR.FMT.XML_UNSUPPORTED_NAMESPACE`; not well-formed →
`SFTR.FMT.XML_NOT_WELLFORMED`).

`TxId` is mandatory (`minOccurs="1"`, unbounded) — a schema-valid
instance **always** carries at least one record. There is **no**
`DataSetActn`/NOTX no-activity branch, so (unlike auth.080 / auth.091 /
auth.106) there is no `*_NO_RECORDS` info path.

## Derive map → `MissingCollateralRecord` (new model)

| Canonical field | Source |
|---|---|
| `uti` | `TxId/UnqTradIdr` (optional in the message) |
| `reporting_counterparty` | `TxId/RptgCtrPty/LEI` |
| `other_counterparty` | `TxId/OthrCtrPty/Lgl/LEI`, else the natural-person `TxId/OthrCtrPty/Ntrl/Id/Id` |
| `master_agreement_type` | `TxId/MstrAgrmt/Tp/Tp` |
| `master_agreement_version` | `TxId/MstrAgrmt/Vrsn` |
| `regime` | `Sftr` |

Reachable checks (one record = one missing-collateral request):

- `SFTR.MCR.MISSING_COLLATERAL_REQUESTED` — one issue per `TxId`
  (Completeness, High): the TR is asking for collateral on this SFT.
- `SFTR.MCR.MISSING_UTI_ON_REQUEST` — fires when `UnqTradIdr` is absent
  (Validity, High): the request cannot be tied to a booked SFT.

See [`../sftr-missing-collateral.md`](../sftr-missing-collateral.md).

## Fields ignored / known unsupported branches

`MstrAgrmt/OthrMstrAgrmtDtls` (free-text master-agreement note — kept
in neither a typed field nor `raw_fields` in v1). All three identifier
*choices* are honoured (`RptgCtrPty/LEI`, `OthrCtrPty/Lgl/LEI`,
`OthrCtrPty/Ntrl/Id/Id`); no other branches exist in this small
message.

### Documented limitations

- **No cross-referencing in v1.** The requested UTIs are surfaced as
  issues but are not (yet) joined against the SFTR history store or a
  companion TSR to confirm whether the firm later supplied the
  collateral. That enrichment is a possible later milestone; v1 is the
  faithful parse + the two `SFTR.MCR.*` checks (same scope discipline
  as the initial auth.080 / auth.106 increments).
- **`OthrMstrAgrmtDtls` is dropped.** The scalar record keeps only the
  master-agreement *type code* and *version*; the free-text "other
  details" string has no consumer and is intentionally not retained.
- **Natural-person other counterparty** is captured as the raw
  `Ntrl/Id/Id` text (no name/birth structure — the schema only carries
  a `Max50Text` id here).
- **No `--store` / no `--prior`.** This is a stateless request scan
  (mirrors `sftr reconcile`'s non-store shape, without persistence).
- **Not a full XSD validation** — same documented "subset" stance as
  the other messages; a fully XSD-valid conformance instance
  (`examples/sftr/conformance/auth083-valid.xml`) is gated by
  `crates/opendqi-xml/tests/xsd_conformance.rs` when `OPENDQI_XSD_DIR`
  is set (see [`../xsd-validation.md`](../xsd-validation.md)).

## Schema source used

ESMA SFTR usage guideline **`auth.083.001.02_ESMAUG_1.0.0`** (base
message `auth.083.001.02`,
`SecuritiesFinancingReportingMissingCollateralRequestV02`). The
SWIFT-licensed XSD is held **locally only** (`ESMA_docs/`, gitignored)
and is **never** redistributed or excerpted; only element names,
nesting and cardinalities were used.

## Verification procedure

1. `cargo test -p opendqi-xml --lib sftr_missing_collateral`
2. `cargo test -p opendqi-xml --test sftr_missing_collateral_integration`
3. `opendqi sftr missing-collateral
   examples/sftr/missing_collateral/auth083-sample.xml --out /tmp/mcr`
   → two `SFTR.MCR.MISSING_COLLATERAL_REQUESTED` (one per `TxId`) and
   one `SFTR.MCR.MISSING_UTI_ON_REQUEST` (the no-UTI record); feeding a
   non-auth.083 namespace → a single
   `SFTR.FMT.XML_UNSUPPORTED_NAMESPACE`.
4. XSD-conformance gate (real ESMA XSD, local-only):
   `OPENDQI_XSD_DIR=<dir> cargo test -p opendqi-xml --test
   xsd_conformance auth083_missing_collateral` — the
   `examples/sftr/conformance/auth083-valid.xml` instance validates
   against the real `auth.083` XSD and the parser round-trips it.
