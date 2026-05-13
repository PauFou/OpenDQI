# OpenDQI EMIR XML format (v0.1, simplified)

OpenDQI accepts a simplified XML representation of EMIR derivative
trade reports for its MVP scan pipeline. The format is intentionally
flat and easy to author — it is **not** ISO 20022 `auth.030`. A
dedicated ISO 20022 adapter is on the roadmap and will live alongside
this parser.

## Namespace

```
https://opendqi.org/schemas/emir/v0.1
```

If a file uses a different namespace, OpenDQI raises an
`EMIR.FMT.XML_UNSUPPORTED_NAMESPACE` warning issue and still tries to
extract trades on a best-effort basis.

## Document layout

```xml
<?xml version="1.0" encoding="UTF-8"?>
<EmirReport xmlns="https://opendqi.org/schemas/emir/v0.1">
  <Header>
    <!-- optional, fields below act as per-trade defaults -->
    <EntityResponsibleForReporting>LEI...</EntityResponsibleForReporting>
    <ReportingTimestamp>2026-05-12T18:00:00Z</ReportingTimestamp>
  </Header>

  <Trade>
    <!-- one record -->
  </Trade>

  <Trade>
    <!-- another record -->
  </Trade>
</EmirReport>
```

Each `<Trade>` becomes one canonical `EmirRecord`. The optional
`<Header>` fills in defaults that individual `<Trade>` elements may
override.

## Supported `<Trade>` children

| Element | Type | Notes |
|---|---|---|
| `<UTI>` | string | Unique Trade Identifier. |
| `<PriorUTI>` | string | Previous UTI when the trade was re-identified. |
| `<ActionType>` | string | EMIR action type (NEWT, MODI, TERM, CORR, ...). |
| `<EventType>` | string | EMIR event type. |
| `<EntityResponsibleForReporting>` | string (LEI) | Overrides the `<Header>` default. |
| `<Counterparty1>` | string (LEI) | |
| `<Counterparty2>` | string (LEI) | |
| `<AssetClass>` | string | |
| `<ProductId>` | string | |
| `<UnderlyingId>` | string | |
| `<Notional currency="...">amount</Notional>` | decimal + ISO 4217 | Currency captured from the attribute. |
| `<Price currency="...">amount</Price>` | decimal + ISO 4217 | |
| `<ExecutionTimestamp>` | RFC 3339 datetime | |
| `<EventTimestamp>` | RFC 3339 datetime | |
| `<ReportingTimestamp>` | RFC 3339 datetime | Overrides `<Header>` default. |
| `<EffectiveDate>` | `YYYY-MM-DD` | |
| `<MaturityDate>` | `YYYY-MM-DD` | |
| `<TerminationDate>` | `YYYY-MM-DD` | When absent or in the future, the trade is considered outstanding. |
| `<Valuation currency="..." timestamp="...">amount</Valuation>` | decimal + ISO 4217 + RFC 3339 | All three values are captured. |
| `<CollateralPortfolioCode>` | string | |
| `<ClearingStatus>` | string | |
| `<CollateralisationCategory>` | string | |

Elements not in this list are **ignored**. OpenDQI emits one
`EMIR.FMT.XML_UNKNOWN_ELEMENT` info issue per distinct element name
per file (so a future schema extension does not flood the report).

## Value parsing rules

- Empty or whitespace-only element bodies are treated as `None`.
- Decimal values use the canonical period `.` decimal separator.
- Dates use `YYYY-MM-DD`.
- Datetimes use RFC 3339 (`2026-02-01T09:00:00Z` or
  `2026-02-01T09:00:00.000Z`).
- A value that fails to parse is logged with `tracing::warn!` and the
  field is left unset, exactly like the CSV ingestion path.

## Format-level issues

The XML reader can raise the following file-scoped issues. They are
written to `issues.csv` and `report.html` alongside record-level
checks.

| Check ID | Severity | Dimension | When |
|---|---|---|---|
| `EMIR.FMT.XML_NOT_WELLFORMED` | Critical | Validity | The file is not well-formed; no trades are extracted. |
| `EMIR.FMT.XML_UNSUPPORTED_NAMESPACE` | Warning | Validity | The root namespace is not `https://opendqi.org/schemas/emir/v0.1`. |
| `EMIR.FMT.XML_UNKNOWN_ELEMENT` | Info | Validity | A `<Trade>` or root child is not part of this schema. Emitted once per distinct name. |

## Example

See `examples/emir/sample.xml` for a complete file covering the same
data-quality patterns as `examples/emir/sample.csv`, and
`examples/emir/broken/malformed.xml` for the well-formedness error
path.

## Schema

A canonical XSD for this format ships in
`examples/emir/schemas/opendqi-emir-v0.1.xsd`. See
[`xsd-validation.md`](xsd-validation.md) for how to plug it into the
scan and validate commands.

## Future work

- ISO 20022 `auth.030.001.xx` adapter (`opendqi-xml::emir::iso20022`).
- ZIP / GZIP archive support.
