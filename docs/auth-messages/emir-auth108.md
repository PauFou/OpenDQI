# EMIR `auth.108` — Derivatives Trade Margin Data Report (MAR)

Per-message coverage note. Parent catalog: [`../auth-messages.md`](../auth-messages.md).
Checks reference: [`../emir-mar-msr.md`](../emir-mar-msr.md).

## Business meaning

The Margin Activity Report is sent by the trade repository to convey
the **margin and collateral activity** reported for derivative
transactions over a period — the margin updates and corrections the TR
processed. It is activity-oriented (what changed), the counterpart of
the Margin *State* Report (`auth.109`, the current margin state).

## Direction

**TR → firm** (made available to the report submitting entity, the
reporting counterparty, and the entity responsible for reporting).

## Coverage status

**schema-verified (subset).** Parser element paths
(`crates/opendqi-xml/src/emir_mar.rs`) are aligned with the real ESMA
EMIR REFIT usage guideline. The bundled fixtures use the real envelope
and element names with synthetic values. Only the field subset the
`EMIR.MAR.*` checks consume is extracted; the message's full tree is
not exhaustively parsed and a fully XSD-valid instance is not asserted.

## Real envelope

```
Document
└─ DerivsTradMrgnDataRpt              (DerivativesTradeMarginDataReportV01)
   ├─ RptHdr/NbRcrds
   └─ TradData            (choice)
      ├─ DataSetActn = "NOTX"          (no-activity / empty report)
      └─ Rpt  (1..500000)  (TradeReport31Choice)
         └─ MrgnUpd | Crrctn           (MarginReportData7 — wrapper = action)
            ├─ RptgTmStmp , EvtDt
            ├─ CtrPtyId/RptgCtrPty/Id/Lgl/Id/LEI
            ├─ CtrPtyId/OthrCtrPty/IdTp/Lgl/Id/LEI
            ├─ TxId/UnqTxIdr
            ├─ Coll/CollPrtflCd/Prtfl/Cd , Coll/CollstnCtgy
            ├─ PstdMrgnOrColl/{InitlMrgnPstdPstHrcut,
            │                  VartnMrgnPstdPstHrcut, XcssCollPstd}(@Ccy)
            └─ RcvdMrgnOrColl/{InitlMrgnRcvdPstHrcut,
                               VartnMrgnRcvdPstHrcut, XcssCollRcvd}(@Ccy)
```

Accepted root namespace:
`urn:iso:std:iso:20022:tech:xsd:auth.108.001.01` (mismatch →
`EMIR.FMT.XML_UNSUPPORTED_NAMESPACE`; non-well-formed →
`EMIR.FMT.XML_NOT_WELLFORMED`).

## Fields extracted (canonical `MarginActivityRecord`)

| Canonical field | Real auth.108 path (relative to `Rpt`) |
|---|---|
| `uti` | `(MrgnUpd\|Crrctn)/TxId/UnqTxIdr` (or `TxId/Prtry/Id`) |
| `counterparty_1` | `…/CtrPtyId/RptgCtrPty/…/LEI` |
| `counterparty_2` | `…/CtrPtyId/OthrCtrPty/…/LEI` |
| `action_type` | derived from the wrapper: `MrgnUpd`→`MRGN`, `Crrctn`→`CORR` |
| `reporting_timestamp` | `…/RptgTmStmp` |
| `event_timestamp` | `…/EvtDt` (an ISO **date**, normalised to `00:00:00Z`) |
| `collateral_portfolio_code` | `…/Coll/CollPrtflCd/Prtfl/Cd` |
| `initial_margin_posted` / `margin_currency` | `…/PstdMrgnOrColl/InitlMrgnPstdPstHrcut` (+`@Ccy`) |
| `initial_margin_collected` | `…/RcvdMrgnOrColl/InitlMrgnRcvdPstHrcut` |
| `variation_margin_posted` | `…/PstdMrgnOrColl/VartnMrgnPstdPstHrcut` |
| `variation_margin_collected` | `…/RcvdMrgnOrColl/VartnMrgnRcvdPstHrcut` |
| `excess_collateral` | `…/PstdMrgnOrColl/XcssCollPstd` (else `RcvdMrgnOrColl/XcssCollRcvd`) |
| `collateral_haircut` | *(none — see Limitations)* |
| `event_type` | *(none — see Limitations)* |

Every other leaf is preserved verbatim in `raw_fields` (path-keyed).

## Fields ignored / known unsupported branches

Pre-haircut amounts (`Initl/VartnMrgn*PreHrcut`), `Coll/CollstnCtgy`
(category not consumed by MAR checks), `Coll/TmStmp`, the full
counterparty detail beyond LEI (nature, broker, submitting agent,
clearing member, beneficiary, entity responsible for reporting), and
any other element of `MarginReportData7` → `raw_fields`.

### Documented limitations

- **"Collected" == schema "received".** OpenDQI's posted/collected
  vocabulary maps onto the schema's posted/**received** sides.
- **Post-haircut is canonical.** The post-haircut amounts are taken as
  the economic values; pre-haircut variants go to `raw_fields`.
- **No haircut percentage.** `auth.108` carries pre/post-haircut
  *amounts*, not a haircut %, so `collateral_haircut` stays `None`.
- **No event datetime.** `auth.108` carries an event *date* (`EvtDt`);
  it is normalised to midnight UTC so `EMIR.MAR.TIMELINESS` remains
  functional (sub-day precision is unavailable).
- **No `event_type`** element exists → left `None`.
- **`EMIR.MAR.MARGIN_TYPE_ENUM` allowed set predates the real
  schema.** It expects `{MARU,MARV,MARC,MARN}`; the real report
  conveys the action via the `TradeReport31Choice` wrapper
  (`MrgnUpd`/`Crrctn` → `MRGN`/`CORR`), so the check fires on every
  record. Refining the check's domain is a separate concern, out of
  this hardening increment's scope.
- **Not a full XSD validation.** Element paths are aligned and
  test-exercised; OpenDQI does not assert an input is a fully valid
  `auth.108.001.01` instance. Use `--xsd` with a locally-held official
  schema for strict validation (see [`../xsd-validation.md`](../xsd-validation.md)).

## Schema source used

ESMA EMIR REFIT *Outgoing Messages* usage guideline
**`auth.108.001.01_ESMAUG_DATMDA_1.1.0`** (base message
`auth.108.001.01`, `DerivativesTradeMarginDataReportV01`). The
SWIFT-licensed XSD is held **locally only** (`ESMA_docs/`, gitignored)
and is **never** redistributed or excerpted; only element names,
nesting and cardinalities were used to align the parser.

## Verification procedure

1. `cargo test -p opendqi-xml --lib emir_mar`
2. `cargo test -p opendqi-xml --test mar_integration` (parse the
   schema-shaped fixture, run all 8 `EMIR.MAR.*`, plus the no-activity
   path).
3. `opendqi emir mar-scan examples/emir/mar/auth108-sample.xml --out /tmp/mar`
   → `mar_report.html` / `mar_issues.csv` / `summary.json` with the
   expected `EMIR.MAR.*` issues; and
   `…/auth108-no-records.xml` → zero records + one
   `EMIR.FMT.MAR_NO_RECORDS` info note, no error.
4. Optional strict check against a locally-held official XSD (not
   committed): `xmllint --noout --schema <local auth.108 xsd> <file>`
   (fixtures are schema-shaped subsets, so a full pass is not
   asserted).
