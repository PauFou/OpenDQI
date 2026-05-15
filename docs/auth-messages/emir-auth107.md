# EMIR `auth.107` — Derivatives Trade State Report (TSR)

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Checks reference: [`../tr-state-checks.md`](../tr-state-checks.md).

## Business meaning

The Derivatives Trade State Report is the trade repository's statement
of the **latest state of every outstanding derivative transaction** it
holds for the recipient. It answers "what does the TR currently believe
is live?" — as opposed to the activity report (`auth.030`, what changed
in a period) or feedback (`auth.092`, what was rejected). It is the
basis for outstanding-trade health, stale/missing valuation detection,
past-maturity-but-still-active detection, duplicate active UTIs, and
book-vs-TR reconciliation.

## Direction

**TR → firm** (made available to the report submitting entity, the
reporting counterparty, and the entity responsible for reporting).

## Coverage status

**schema-verified (subset).** The parser's element paths
(`crates/opendqi-xml/src/tr_state.rs`) are aligned with the real ESMA
EMIR REFIT usage-guideline schema (see *Schema source*). The bundled
fixtures use the real envelope and element names with synthetic values.
Only the field subset OpenDQI's state-health checks consume is
extracted; the message's full tree is **not** exhaustively parsed and a
fully XSD-valid instance is **not** asserted (every mandatory branch of
the message would have to be populated — out of scope). The limits are
listed below explicitly.

## Real envelope

```
Document
└─ DerivsTradStatRpt                     (DerivativesTradeStateReportV01)
   ├─ RptHdr/NbRcrds                      (declared record count)
   └─ TradData            (choice)
      ├─ DataSetActn = "NOTX"             (no-activity / empty report)
      └─ Stat  (1..500000, repeating)     (one per trade state)
         ├─ CtrPtySpcfcData
         │  ├─ CtrPty/RptgCtrPty/Id/Lgl/Id/LEI
         │  ├─ CtrPty/OthrCtrPty/IdTp/Lgl/Id/LEI
         │  ├─ Valtn/CtrctVal/Amt (@Ccy) , Valtn/TmStmp
         │  └─ RptgTmStmp                  (per-record "state as of")
         └─ CmonTradData/TxData
            ├─ TxId/UnqTxIdr
            ├─ NtnlAmt/FrstLeg/Amt/Amt (@Ccy)
            ├─ FctvDt , XprtnDt , EarlyTermntnDt
            └─ CollPrtflCd/Prtfl/Cd
```

Accepted root namespace: `urn:iso:std:iso:20022:tech:xsd:auth.107.001.01`
(a mismatching namespace yields `EMIR.FMT.XML_UNSUPPORTED_NAMESPACE`;
non-well-formed XML yields `EMIR.FMT.XML_NOT_WELLFORMED`).

## Fields extracted (canonical `TrStateRecord`)

| Canonical field | Real auth.107 path (relative to `Stat`) |
|---|---|
| `uti` | `CmonTradData/TxData/TxId/UnqTxIdr` (or `TxId/Prtry/Id`) |
| `reporting_counterparty` | `CtrPtySpcfcData/CtrPty/RptgCtrPty/…/LEI` |
| `other_counterparty` | `CtrPtySpcfcData/CtrPty/OthrCtrPty/…/LEI` |
| `notional_amount` / `notional_currency` | `CmonTradData/TxData/NtnlAmt/FrstLeg/Amt/Amt` (+ `@Ccy`) |
| `valuation_amount` / `valuation_currency` | `CtrPtySpcfcData/Valtn/CtrctVal/Amt` (+ `@Ccy`) |
| `valuation_timestamp` | `CtrPtySpcfcData/Valtn/TmStmp` |
| `state_as_of` | `CtrPtySpcfcData/RptgTmStmp` (per record) |
| `effective_date` | `CmonTradData/TxData/FctvDt` |
| `maturity_date` | `CmonTradData/TxData/XprtnDt` |
| `termination_date` | `CmonTradData/TxData/EarlyTermntnDt` |
| `collateral_portfolio_code` | `CmonTradData/TxData/CollPrtflCd/Prtfl/Cd` |
| `status` | *(no schema element — see Limitations)* |

Every other leaf inside a `Stat` record is preserved verbatim in
`raw_fields` (path-keyed) so nothing is silently dropped.

## Fields ignored / known unsupported branches

The following are present in the real message but **not** extracted
into the canonical model (they land in `raw_fields` or are skipped):

- The bulk of `TradeTransaction49` beyond the fields above: price,
  notional quantity, delivery type, execution timestamp, settlement
  dates, master agreement, compression / PTRR flags, derivative events,
  trade confirmation, clearing, interest-rate / commodity / FX / option
  / energy / credit asset-class blocks, other payments, package.
- The second notional leg (`NtnlAmt/ScndLeg`) and notional schedules
  (`SchdlPrd`) — only the first-leg headline amount is taken.
- Counterparty detail beyond the LEI: nature (FI/NFI sector), trading
  capacity, direction/side, broker, submitting agent, clearing member,
  beneficiary, entity responsible for reporting.
- `Lvl = PSTN` (position-level) economics; collateral / margin detail;
  `TechAttrbts`; `CtrctData` / `CtrctMod` contract metadata.

### Documented limitations

- **No status element.** `auth.107` carries no per-trade status field;
  presence of a record in a Trade State Report *is* the TR's outstanding
  state. `status` is therefore left `None`, and the state-health checks
  treat `None` as outstanding. (`is_outstanding` in
  `crates/opendqi-core/src/dq/tr_state/mod.rs` is unchanged.)
- **No header "state as of".** The real header (`RptHdr`) only carries
  `NbRcrds`. `state_as_of` is sourced from each record's
  `CtrPtySpcfcData/RptgTmStmp`. If a record omits it, `state_as_of`
  stays `None` and the staleness / past-maturity checks degrade
  gracefully (they already gate on `Option`).
- **No-activity report.** `TradData/DataSetActn = "NOTX"` yields zero
  records plus one informational `EMIR.FMT.TSR_NO_RECORDS` note (not an
  error).
- **Not a full XSD validation.** Element paths are aligned with the
  real schema and exercised by tests; OpenDQI does not assert that an
  input is a fully valid `auth.107.001.01` instance. Use `--xsd` with a
  locally-held official schema for strict validation (see
  [`../xsd-validation.md`](../xsd-validation.md)).

## Schema source used

ESMA EMIR REFIT *Outgoing Messages* usage guideline
**`auth.107.001.01_ESMAUG_DATTSR_1.1.0`** (base message
`auth.107.001.01`, `DerivativesTradeStateReportV01`), published
2023-11-02. The SWIFT-licensed XSD is held **locally only**
(`ESMA_docs/`, gitignored) and is **never** redistributed or excerpted;
only element names, nesting and cardinalities were used to align the
parser. No verbatim schema text appears in this repository.

## Verification procedure

1. Parser + model mapping unit tests:
   `cargo test -p opendqi-xml --lib tr_state`
2. End-to-end (parse the schema-shaped fixture, run all seven
   `EMIR.TST.*` checks, plus the no-activity path):
   `cargo test -p opendqi-xml --test tr_state_integration`
3. CLI smoke:
   `opendqi emir tr-state-scan examples/emir/tr_state/auth107-sample.xml --out /tmp/tsr`
   → `tr_state_report.html` / `tr_state_issues.csv` / `summary.json`
   with the expected `EMIR.TST.*` issues; and
   `opendqi emir tr-state-scan examples/emir/tr_state/auth107-no-records.xml --out /tmp/tsr0`
   → zero records, one `EMIR.FMT.TSR_NO_RECORDS` info note, no error.
4. Optional strict check against a locally-held official XSD (not
   committed): `xmllint --noout --schema <local auth.107 xsd> <file>`.
   Note that the bundled fixtures are *schema-shaped* (subset), so a
   full `xmllint --schema` pass is not asserted by the test suite.
