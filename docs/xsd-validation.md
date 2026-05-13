# XSD validation

OpenDQI can validate XML inputs against an XML Schema (XSD) and
surface schema violations as data-quality issues. Validation is
delegated to the `xmllint` binary (part of `libxml2`); OpenDQI does
not embed its own XSD engine.

## Requirements

`xmllint` must be available on `PATH`. On macOS it is preinstalled in
`/usr/bin/xmllint`; on Debian/Ubuntu install `libxml2-utils`. Other
platforms typically package it as `libxml2` or similar.

## Canonical schema

The schema for the simplified OpenDQI EMIR XML format (see
[`xml-format.md`](xml-format.md)) ships in the repo:

```
examples/emir/schemas/opendqi-emir-v0.1.xsd
```

Bring your own schema by passing any other XSD file to `--xsd`.

## During a scan

```bash
opendqi emir scan ./data/emir.xml \
  --xsd ./examples/emir/schemas/opendqi-emir-v0.1.xsd \
  --out ./report
```

When `--xsd` is set, every XML input in the batch is validated. CSV
inputs are not affected. Each schema violation becomes one issue:

- check id: `EMIR.FMT.XSD_VIOLATION`
- severity: `high`
- dimension: `validity`

In addition to the usual `summary.json`, `issues.csv`, and
`report.html`, the scan writes a dedicated `xsd_errors.csv`:

| Column | Content |
|---|---|
| `source_file` | path of the validated XML |
| `line` | 1-based line where xmllint reported the error |
| `column` | (currently empty) |
| `message` | message taken verbatim from xmllint |

If `xmllint` is missing or the schema cannot be loaded, OpenDQI
records one `EMIR.FMT.XSD_TOOL_ERROR` issue (severity `warning`) per
file and continues — the scan does not abort.

## Pure validation (no DQ checks)

```bash
opendqi emir validate ./data/emir.xml \
  --xsd ./examples/emir/schemas/opendqi-emir-v0.1.xsd
```

Useful in CI. The command:

1. Streams a well-formedness check on every input.
2. Validates each well-formed XML against the schema.
3. Prints errors to stderr in `path:line: message` form.
4. Prints a one-line summary to stdout.
5. Exits `0` if the input is well-formed and schema-conforming;
   exits `1` otherwise.

`--xsd` is required. The input may be a single XML file or a
directory of XML files (CSV inputs are rejected; use `scan` for
CSV+XML mixed pipelines).

## Severity rationale

Schema violations are `high`, not `critical`. A non-conforming
document can usually be repaired upstream and resubmitted. `critical`
is reserved for cases where there is no record at all to remediate
(malformed XML, duplicate UTI within the same active state).
