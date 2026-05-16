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

## XSD-conformance reliability gate

The lean fixtures under `examples/{emir,sftr}/<layer>/` are
deliberately **schema-shaped subsets** (fast unit/golden/robustness
tests) — they are *not* fully XSD-valid and never claim to be.

In addition, each schema-verified message ships a **fully
XSD-valid** conformance fixture under
`examples/{emir,sftr}/conformance/auth<NNN>-valid.xml`. The gate
`crates/opendqi-xml/tests/xsd_conformance.rs` proves, per message,
that this instance (a) validates against the **real ESMA XSD** via
`xmllint` and (b) is ingested by the OpenDQI parser producing records
with no format issues.

This gate is **developer/preflight-local and never runs in public
CI**: the real ESMA XSDs are SWIFT-licensed and gitignored
(`ESMA_docs/`), so they are not present in CI. Each case **self-skips**
(prints a notice, passes as a no-op) unless both:

- `xmllint` is on `PATH` (`libxml2-utils` on Debian/Ubuntu;
  preinstalled on macOS), and
- `OPENDQI_XSD_DIR` points at a directory of **extracted real ESMA
  `.xsd` files** you hold locally (never commit them).

Prepare it once (the ESMA usage-guideline XSDs are self-contained, so
a flat directory of `.xsd` files is enough; filenames may carry ESMA
version suffixes — the gate resolves by `auth.NNN` prefix):

```bash
mkdir -p /tmp/esma-xsd
unzip -j "ESMA_docs/EMIR/EMIR Refit Outgoing Messages v1.0.0.zip" '*.xsd' -d /tmp/esma-xsd
unzip -j "ESMA_docs/SFTR/SFTR Reporting Mar 2023.zip"            '*.xsd' -d /tmp/esma-xsd
OPENDQI_XSD_DIR=/tmp/esma-xsd cargo test -p opendqi-xml --test xsd_conformance
```

`./scripts/preflight.sh` runs the gate automatically when
`OPENDQI_XSD_DIR` is exported, and prints how to enable it otherwise.
