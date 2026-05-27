# EMIR `auth.031` — Financial Instrument Reporting Status Advice

Per-message coverage note. Parent catalog:
[`../auth-messages.md`](../auth-messages.md).
Command: `opendqi emir status-advice-scan <auth031.xml> --out ./out/`
(added in v0.20 Phase B6).
Python: `opendqi.emir.status_advice_scan(path)` (added in v0.20 Phase C6).

## Business meaning

`auth.031` is the **TR → firm status acknowledgement** the TR
sends to the firm after receiving a submission (typically an
auth.030 TAR but applicable to any reportable submission). It
confirms whether the TR accepted, rejected, or is still processing
that submission. Each envelope can carry many per-submission acks
(typically one batch ack per submission window).

**Honest scope**: there is no business DQ signal in this message
beyond an envelope sanity check. The ack carries no derivatives
payload — only the identifier of the submission being acked, the
status enum (ACPT/ACTC/RJCT/PDNG/PRTL), an ack timestamp, and an
optional TR-specific error code on RJCT acks. Manqué ack = problem
of connectivity / submission round-trip, not problem of data
quality. The TR-specific rejection reasons are far better
captured by `auth.092` (Derivatives Trade Rejection Statistical
Report) which is OpenDQI's first-class feedback message.

We ship one structural check that verifies the envelope identity
(submission_id + ack_status + ack_timestamp present), which is the
entire DQ surface for this message. Adopters looking for deeper
signal on rejection reasons and per-rule rejection rates should
use the `opendqi emir feedback` workflow on the corresponding
`auth.092` files instead.

## XSD envelope

```text
Document (urn:iso:std:iso:20022:tech:xsd:auth.031.001.01)
└─ FinInstrmRptgStsAdvc
   ├─ MsgHdr
   │  └─ CreDtTm        (envelope timestamp; fallback for per-ack)
   └─ StsAdvc[]         (one or more per-submission acks)
      ├─ OrgnlMsgId     (id of the submission being acked)
      ├─ Sts            (status string: ACPT/ACTC/RJCT/PDNG/PRTL)
      ├─ ErrCd?         (TR-specific error code, present on RJCT)
      └─ CreDtTm?       (per-ack timestamp, wins over envelope)
```

v0.20 extracts (per record, one record per `StsAdvc`):

- `record_id` synthesised as `<source>#Ack-<1-based-index>`
- `submission_id` from `OrgnlMsgId` (or `MsgId` as fallback)
- `ack_status` from `Sts` — stored as `Option<String>` (not
  enum'd) for forward-compatibility with TR-specific code
  variants; the sanity check verifies presence, not value
- `ack_timestamp` from per-ack `CreDtTm` when present, otherwise
  falls back to envelope `MsgHdr/CreDtTm`
- `error_code` from `ErrCd` (present on RJCT acks; TR
  dictionary-specific, not enum'd)

## DQI coverage

**None.** auth.031 contributes 0 indicators to the EMIR Data
Quality Pack. The message is structural — see "Honest scope"
above and the `opendqi emir feedback` workflow for the
first-class rejection-feedback signal.

## Granular checks (1 `EMIR.ACK.*`, v0.20 A5)

| Check ID | Dimension | Severity |
|---|---|---|
| `EMIR.ACK.ENVELOPE_WELLFORMED` | validity | critical |

The check fires when **any** of `submission_id`, `ack_status`, or
`ack_timestamp` is missing — an ack without any of the three is
not operationally useful for SLA / timeliness measurement. The
`field` slot lists every missing key joined.

## Scope notes (v0.20)

- Multiple acks per envelope (auth.031 IS a list message: one
  envelope = N per-submission acks).
- Per-ack `CreDtTm` wins over envelope `MsgHdr/CreDtTm` when both
  are present; the envelope provides a fallback.
- `ack_status` kept as opaque string for forward-compat with
  TR-specific code dictionaries (the sanity check verifies
  presence not value).
- No DQI pack integration (no roll-up indicator).
- Standalone `opendqi emir status-advice-scan` subcommand ships
  in v0.20 Phase B6 alongside the equivalent Python wrapper.
