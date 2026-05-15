# SFTR `auth.079` — Securities Financing Transaction State Report (SFTR TSR)

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Checks reference: [`../sftr-checks.md`](../sftr-checks.md).

## Business meaning

The Securities Financing Transaction State Report is the trade
repository's statement of the **current state of every outstanding
SFT** it holds for the recipient — repos, buy/sell-backs, securities
lending and margin lending. It is the SFTR equivalent of EMIR
`auth.107`: what the TR currently believes is live, used for
outstanding-SFT health, missing/stale collateral, past-maturity-still-
active, duplicate active UTIs, and margin-lending (MGLD) state checks.

## Direction

**TR → firm** (made available to the report submitting entity, the
reporting counterparty, and the entity responsible for reporting).

## Coverage status

**schema-verified (subset).** Parser element paths
(`crates/opendqi-xml/src/sftr_tr_state.rs`) are aligned with the real
ESMA SFTR usage guideline. Fixtures use the real envelope and element
names with synthetic values. Only the field subset the 6 `SFTR.TST.*`
+ 6 `SFTR.MSR.MGLD_*` checks consume is extracted; a fully XSD-valid
instance is not asserted.

## Real envelope

```
Document
└─ SctiesFincgRptgTxStatRpt   (SecuritiesFinancingReportingTransactionStateReportV02)
   └─ TradData  (TradeStateReport5Choice — choice)
      ├─ DataSetActn = "NOTX"          (no-activity / empty report)
      └─ Stat  (1..n)  (TradeStateReport16)
         ├─ TechRcrdId
         ├─ CtrPtySpcfcData
         │  ├─ RptgDtTm                 (per-record "state as of")
         │  └─ CtrPty
         │     ├─ RptgCtrPty/Id/LEI
         │     └─ OthrCtrPty/Id/Lgl/LEI
         ├─ LnData  (TransactionLoanData31Choice — 4-way, wrapper = SFT type)
         │  ├─ RpTrad     → REPO ┐ UnqTradIdr, EvtDt, ValDt,
         │  ├─ BuySellBck → BSBC ┤ Term/Fxd/MtrtyDt, TermntnDt,
         │  │                    │ PrncplAmt/ValDtAmt(@Ccy)
         │  ├─ SctiesLndg → SLEB ┤ … LnVal(@Ccy)
         │  └─ MrgnLndg   → MGLD ┘ … OutsdngMrgnLnAmt(@Ccy)
         ├─ CollData  (TransactionCollateralData18Choice — 4-way)
         │  └─ … Security52/55: Id=ISIN, MktVal/Amt(@Ccy),
         │     HrcutOrMrgn, AvlblForCollReuse
         └─ CtrctMod/ActnTp  (NEWT/MODI/CORR/ETRM/VALU/COLU/POSC/EROR)
```

`auth.079` has **no header** and **no per-trade status element**: a
record in a Trade State Report *is* outstanding. `state_as_of` is the
per-record `CtrPtySpcfcData/RptgDtTm`. Accepted root namespace:
`urn:iso:std:iso:20022:tech:xsd:auth.079.001.02` (mismatch →
`SFTR.FMT.XML_UNSUPPORTED_NAMESPACE`).

## Fields extracted (canonical `SftrTrStateRecord`)

| Canonical field | Real auth.079 path (relative to `Stat`) |
|---|---|
| `uti` | `LnData/{RpTrad\|BuySellBck\|SctiesLndg\|MrgnLndg}/UnqTradIdr` |
| `sft_type` | derived from the loan-branch wrapper: `RpTrad`→`REPO`, `BuySellBck`→`BSBC`, `SctiesLndg`→`SLEB`, `MrgnLndg`→`MGLD` |
| `reporting_counterparty` | `CtrPtySpcfcData/CtrPty/RptgCtrPty/Id/LEI` |
| `other_counterparty` | `CtrPtySpcfcData/CtrPty/OthrCtrPty/Id/Lgl/LEI` |
| `state_as_of` | `CtrPtySpcfcData/RptgDtTm` |
| `loan_value` / `loan_currency` | `…/PrncplAmt/ValDtAmt` (repo/BSB) \| `…/LnVal` (SecLn) \| `…/OutsdngMrgnLnAmt` (MgnLn) (+`@Ccy`) |
| `effective_date` | `LnData/*/EvtDt` |
| `settlement_date` | `LnData/*/ValDt` |
| `maturity_date` | `LnData/*/Term/Fxd/MtrtyDt` (open-term → none) |
| `termination_date` | `LnData/*/TermntnDt` |
| `collateral_value` / `collateral_currency` | `CollData/…/MktVal/Amt` (+`@Ccy`) |
| `haircut` | `CollData/…/HrcutOrMrgn` |
| `collateral_isin` | `CollData/…/Id` (first/representative security component) |
| `reuse_indicator` | `CollData/…/AvlblForCollReuse` |
| `status` | *(no schema element — record present == outstanding)* |
| `collateral_portfolio_code` | *(none — see Limitations)* |

Every other leaf is preserved verbatim in `raw_fields`.

## Fields ignored / known unsupported branches

`CtrctMod/ActnTp`, `TechRcrdId`, `RcncltnFlg`, the second
`PrncplAmt/MtrtyDtAmt` leg, cash/commodity collateral, interest-rate /
rebate / lending-fee detail, master agreement, clearing, basket
identifiers, and any collateral security component **beyond the first
representative one** → `raw_fields`.

### Documented limitations

- **No per-trade status element.** `status` stays `None`; the checks
  treat `None` as outstanding (correct TSR semantics; `is_outstanding`
  in `dq/sftr/tr_state/mod.rs` unchanged).
- **SFT type from the loan-choice wrapper** (`RpTrad`/`BuySellBck`/
  `SctiesLndg`/`MrgnLndg` → `REPO`/`BSBC`/`SLEB`/`MGLD`) — there is no
  free SFT-type element. The 6 `SFTR.MSR.MGLD_*` checks gate on
  `sft_type == "MGLD"`, i.e. the `MrgnLndg` branch.
- **First collateral component only.** A record may carry many
  collateral securities; only the first/representative component's
  market value, currency, haircut and ISIN are extracted (rest →
  `raw_fields`).
- **No collateral portfolio code.** `auth.079` carries no SFT
  collateral-portfolio code at record level (the only `PrtflCd` is
  clearing-specific). `collateral_portfolio_code` stays `None`, so
  `SFTR.MSR.MGLD_REUSE_REQUIRES_PORTFOLIO` fires on every MGLD record
  with `AvlblForCollReuse=true`. Refining that check is a separate
  out-of-scope concern.
- **Open-term SFTs have no maturity date** (`Term/Opn`); only
  fixed-term (`Term/Fxd/MtrtyDt`) populates `maturity_date`.
- **Not a full XSD validation.** Element paths are aligned and
  test-exercised; OpenDQI does not assert an input is a fully valid
  `auth.079.001.02` instance. Use `--xsd` with a locally-held official
  schema for strict validation (see [`../xsd-validation.md`](../xsd-validation.md)).

## Schema source used

ESMA SFTR *TR-to-Authority data exchange* usage guideline
**`auth.079.001.02_ESMAUG_SFTTRS_1.1.0`** (base message
`auth.079.001.02`,
`SecuritiesFinancingReportingTransactionStateReportV02`). The
SWIFT-licensed XSD is held **locally only** (`ESMA_docs/`, gitignored)
and is **never** redistributed or excerpted; only element names,
nesting and cardinalities were used to align the parser.

## Verification procedure

1. `cargo test -p opendqi-xml --lib sftr_tr_state`
2. `cargo test -p opendqi-xml --test sftr_tr_state_integration`
   (parse the schema-shaped fixture, run all 12 reachable
   `SFTR.TST.*`/`SFTR.MSR.MGLD_*`, plus the no-activity path).
3. `opendqi sftr tr-state-scan examples/sftr/tr_state/auth079-sample.xml --out /tmp/s`
   → `tr_state_report.html` / `tr_state_issues.csv` / `summary.json`
   with the expected issues; and
   `…/auth079-no-records.xml` → zero records + one
   `SFTR.FMT.SFTR_TSR_NO_RECORDS` info note, no error.
4. Optional strict check against a locally-held official XSD (not
   committed): `xmllint --noout --schema <local auth.079 xsd> <file>`
   (fixtures are schema-shaped subsets, so a full pass is not
   asserted).
