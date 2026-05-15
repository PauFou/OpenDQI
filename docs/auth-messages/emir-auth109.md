# EMIR `auth.109` — Derivatives Trade Margin Data Transaction State Report (MSR)

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Checks reference: [`../emir-mar-msr.md`](../emir-mar-msr.md).

## Business meaning

The Margin State Report is the trade repository's statement of the
**current margin and collateral state** for outstanding derivative
transactions — what margin is posted/received right now, per trade. It
is state-oriented, the counterpart of the Margin *Activity* Report
(`auth.108`, what changed over a period).

## Direction

**TR → firm** (made available to the report submitting entity, the
reporting counterparty, and the entity responsible for reporting).

## Coverage status

**schema-verified (subset).** Parser element paths
(`crates/opendqi-xml/src/emir_msr.rs`) are aligned with the real ESMA
EMIR REFIT usage guideline. Fixtures use the real envelope and element
names with synthetic values. Only the field subset the `EMIR.MSR.*`
checks consume is extracted; a fully XSD-valid instance is not
asserted.

## Real envelope

```
Document
└─ DerivsTradMrgnDataTxStatRpt   (…TradeMarginDataTransactionStateReportV01)
   ├─ RptHdr/NbRcrds
   └─ TradData            (choice)
      ├─ DataSetActn = "NOTX"          (no-activity / empty report)
      └─ Stat  (1..500000)             (MarginReportData8)
         ├─ RptgTmStmp                  (per-record "state as of"; optional)
         ├─ CtrPtyId/RptgCtrPty/Id/Lgl/Id/LEI
         ├─ CtrPtyId/OthrCtrPty/IdTp/Lgl/Id/LEI
         ├─ TxId/UnqTxIdr
         ├─ Coll/CollPrtflCd/Prtfl/Cd , Coll/CollstnCtgy
         ├─ PstdMrgnOrColl/{InitlMrgnPstdPstHrcut,
         │                  VartnMrgnPstdPstHrcut, XcssCollPstd}(@Ccy)
         ├─ RcvdMrgnOrColl/{InitlMrgnRcvdPstHrcut,
         │                  VartnMrgnRcvdPstHrcut, XcssCollRcvd}(@Ccy)
         └─ CtrctMod/ActnTp
```

Like `auth.107`, the header carries only `NbRcrds` (no `StateAsOf`);
`state_as_of` is sourced per record from `RptgTmStmp` (optional in
`auth.109` — when absent, time-based checks degrade gracefully).
Accepted root namespace:
`urn:iso:std:iso:20022:tech:xsd:auth.109.001.01`.

## Fields extracted (canonical `MarginStateRecord`)

| Canonical field | Real auth.109 path (relative to `Stat`) |
|---|---|
| `uti` | `TxId/UnqTxIdr` (or `TxId/Prtry/Id`) |
| `counterparty_1` | `CtrPtyId/RptgCtrPty/…/LEI` |
| `counterparty_2` | `CtrPtyId/OthrCtrPty/…/LEI` |
| `state_as_of` | `RptgTmStmp` (per record) |
| `collateral_portfolio_code` | `Coll/CollPrtflCd/Prtfl/Cd` |
| `collateralization_category` | `Coll/CollstnCtgy` |
| `initial_margin_posted_current` / `margin_currency` | `PstdMrgnOrColl/InitlMrgnPstdPstHrcut` (+`@Ccy`) |
| `initial_margin_collected_current` | `RcvdMrgnOrColl/InitlMrgnRcvdPstHrcut` |
| `variation_margin_posted_current` | `PstdMrgnOrColl/VartnMrgnPstdPstHrcut` |
| `variation_margin_collected_current` | `RcvdMrgnOrColl/VartnMrgnRcvdPstHrcut` |
| `collateral_market_value` | `RcvdMrgnOrColl/XcssCollRcvd` (closest analog) |
| `haircut_applied` | *(none — see Limitations)* |

Every other leaf (incl. `CtrctMod/ActnTp`, `EvtDt`, pre-haircut
amounts, `XcssCollPstd`, `Coll/TmStmp`) is preserved verbatim in
`raw_fields`.

## Fields ignored / known unsupported branches

`CtrctMod/ActnTp` (no model field; not consumed by the 8 `EMIR.MSR.*`
checks), pre-haircut amounts, posted excess collateral, `EvtDt`,
`Coll/TmStmp`, full counterparty detail beyond LEI, and any other
element of `MarginReportData8` → `raw_fields`.

### Documented limitations

- **"Collected" == schema "received"**; **post-haircut is canonical**
  (pre-haircut → `raw_fields`) — as in `auth.108`.
- **No single "collateral market value".** `collateral_market_value`
  is sourced from received excess collateral (`XcssCollRcvd`), the
  closest economic analog; absent → `raw_fields`.
- **No haircut percentage.** `auth.109` carries no haircut %, so
  `haircut_applied` stays `None` and **`EMIR.MSR.HAIRCUT_OUT_OF_RANGE`
  is unreachable with real data** (it is therefore not asserted by the
  e2e test).
- **`EMIR.MSR.COLLATERALIZATION_CATEGORY_ENUM` allowed set predates
  the real schema.** It expects `{FCOL,PCOL,UCOL,OCOL}`; the real ESMA
  codes are `{FLCL,OWCL,PRCL,UNCL}`, so the check fires on every record
  carrying a category. Refining the check's domain is a separate
  concern, out of this hardening increment's scope.
- **No header state-as-of.** Sourced per record from `RptgTmStmp`;
  when omitted, `state_as_of` is `None` and `EMIR.MSR.MARGIN_STALE`
  degrades gracefully.
- **Not a full XSD validation** (same stance as `auth.108`).

## Schema source used

ESMA EMIR REFIT *Outgoing Messages* usage guideline
**`auth.109.001.01_ESMAUG_DATMDS_1.1.0`** (base message
`auth.109.001.01`,
`DerivativesTradeMarginDataTransactionStateReportV01`). The
SWIFT-licensed XSD is held **locally only** (`ESMA_docs/`, gitignored),
**never** redistributed or excerpted.

## Verification procedure

1. `cargo test -p opendqi-xml --lib emir_msr`
2. `cargo test -p opendqi-xml --test msr_integration` (parse the
   schema-shaped fixture, run the seven reachable `EMIR.MSR.*` checks,
   plus the no-activity path).
3. `opendqi emir msr-scan examples/emir/msr/auth109-sample.xml --out /tmp/msr`
   → `msr_report.html` / `msr_issues.csv` / `summary.json`; and
   `…/auth109-no-records.xml` → zero records + one
   `EMIR.FMT.MSR_NO_RECORDS` info note, no error.
4. Optional strict check against a locally-held official XSD (not
   committed): `xmllint --noout --schema <local auth.109 xsd> <file>`
   (fixtures are schema-shaped subsets).
