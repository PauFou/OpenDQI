# EMIR `auth.029` — Derivatives Trade Report Query

Per-message coverage note. Parent catalog:
[`../auth-messages.md`](../auth-messages.md).
Command: `opendqi emir query-scan <auth029.xml> --out ./out/`
(added in v0.20 Phase B6).
Python: `opendqi.emir.query_scan(path)` (added in v0.20 Phase C6).

## Business meaning

`auth.029` is the **firm-side query envelope** the firm sends *to*
the TR to retrieve its own derivatives data. Unlike the report
messages OpenDQI normally ingests (TR → firm flows: auth.030,
auth.107, auth.092, auth.106, …), auth.029 carries no derivatives
payload itself — only the identifier of the requesting firm, the
timestamp of the request, and free-form descriptions of the filter
criteria that scope which trades the TR should return.

**Honest scope**: there is no business DQ signal in this message
beyond an envelope sanity check. The query is a request, not a
report; the response (and the timeliness / completeness of that
response) is a TR-side operational matter that OpenDQI cannot
observe in isolation. We ship one structural check that verifies
the envelope identity (query id + requesting LEI present), which
is the entire DQ surface for this message. Adopters looking for
deeper signal on TR responsiveness should pair this scan with the
corresponding TR-side data (typically retrieved via the same query
mechanism) and use the standard auth.030 / auth.107 scanners on
the returned payload.

## XSD envelope

```text
Document (urn:iso:std:iso:20022:tech:xsd:auth.029.001.04)
└─ DerivsTradRptQry
   ├─ MsgHdr
   │  ├─ MsgId       (query id, string)
   │  └─ CreDtTm     (request timestamp, ISODateTime)
   ├─ TradRptQryCrit[]  (one or more filter blocks)
   │  └─ …               (date range, UTI list, CP filters, …)
   └─ RqstngPty
      └─ Id/.../LEI  (LEI of the requesting firm)
```

The parser collapses each `TradRptQryCrit` block into one opaque
string in `filter_descriptions` (format
`tail/leaf=value;tail/leaf=value`). Everything else lands in
`raw_fields` for downstream inspection. The full XSD grammar
allows many alternates inside `TradRptQryCrit` (date ranges, UTI
lists, counterparty filters, asset-class filters, …); the opaque
string is sufficient for audit-trail purposes and keeps the
parser stable against XSD revisions.

v0.20 extracts (per record, one record per envelope):

- `record_id` synthesised as `<source>#Qry-1`
- `query_id` from `MsgHdr/MsgId` (Option — may be absent per XSD)
- `query_timestamp` from `MsgHdr/CreDtTm` (parsed as RFC3339)
- `requesting_lei` from `RqstngPty/.../LEI`
- `filter_descriptions` — one Vec<String> entry per
  `TradRptQryCrit` block

## DQI coverage

**None.** auth.029 contributes 0 indicators to the EMIR Data
Quality Pack. The message is structural; rolling up "share of
queries missing requesting_lei" across firms would be more noise
than signal.

## Granular checks (1 `EMIR.QRY.*`, v0.20 A5)

| Check ID | Dimension | Severity |
|---|---|---|
| `EMIR.QRY.ENVELOPE_WELLFORMED` | validity | critical |

The check fires when **either** `query_id` or `requesting_lei` is
missing — a query without either is not a meaningful regulatory
request. The `field` slot lists every missing key joined.

## Scope notes (v0.20)

- One record per envelope (auth.029 is not a list message).
- Per-`TradRptQryCrit` content captured as one opaque string each;
  no field-by-field promotion of filter alternates.
- No DQI pack integration (no roll-up indicator).
- Standalone `opendqi emir query-scan` subcommand ships in v0.20
  Phase B6 alongside the equivalent Python wrapper.
