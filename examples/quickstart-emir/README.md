# OpenDQI — EMIR quickstart kit

Three synthetic ISO 20022 EMIR samples, zipped together so you can
try the three primary OpenDQI workflows without first hunting
through the rest of the `examples/` tree.

**All three are synthetic.** No real client or trade-repository
data is included anywhere in this repo (see `CLAUDE.md` and the
project security policy).

## Files

| File | Message | What it is | Issues fired |
|---|---|---|---|
| `auth107-tsr.xml` | `auth.107.001.01` | EMIR Trade State Report — 8 outstanding derivatives, including one with a stale valuation, one past maturity, one duplicate-active UTI, one placeholder maturity `9999-12-31`, one valuation post-termination, one with missing valuation. | **16** (2 critical / 5 high / 1 warning / 8 info), quality score **86.6** |
| `auth030-tar.xml` | `auth.030.001.03` | EMIR Trade Activity Report — 5 records covering NEWT/MODI/TERM, including a duplicate NEWT in the same batch and a repeated-correction pattern. | **7** (mostly `EMIR.TRA.*` patterns + duplicate-NEWT) |
| `auth092-feedback.xml` | `auth.092.001.04` | EMIR Rejection Statistical Report — 2 UTIs rejected by the TR, validation rule codes `VR-0001`, `VR-0042`, `VR-0100`. | **2** critical `EMIR.FBK.TR_REJECTED_UTI`, quality score **75.0** |

These are the same files exercised by the golden suite
(`crates/opendqi-cli/tests/golden/emir-{tr-state,tr-activity,
feedback,tr-audit}.{summary.json,issues.csv}`), so the counts
above are byte-pinned in CI.

## Run the three workflows

### TR state health

```bash
opendqi emir tr-state-scan auth107-tsr.xml --out ./report/
open ./report/tr_state_report.html         # macOS
# xdg-open ./report/tr_state_report.html   # Linux
```

### Rejection intelligence

```bash
opendqi emir feedback auth092-feedback.xml --out ./feedback/
open ./feedback/report.html
```

### Combined audit (all three layers + 3 cross-layer coherence checks)

```bash
opendqi emir tr-audit \
  --tar auth030-tar.xml \
  --tsr auth107-tsr.xml \
  --feedback auth092-feedback.xml \
  --out ./audit/
open ./audit/tr_audit_report.html
```

## One-shot

[`scripts/demo.sh`](../../scripts/demo.sh) at the repo root runs
all three commands above for you against this kit, in under 30s on
a recent laptop.

## Further reading

- [`docs/use-cases.md`](../../docs/use-cases.md) — the operator
  scenarios behind each workflow, with the expected output broken
  down by severity / dimension.
- [`docs/positioning.md`](../../docs/positioning.md) — the
  three-layer mental model (TAR / TSR / Rejection) the product is
  built around.
- [`docs/emir-checks.md`](../../docs/emir-checks.md) — the full
  151-check EMIR catalog.
- [`examples/emir/`](../emir/) — the rest of the EMIR fixture tree
  (margin state, warnings, recon stats, collateral audit,
  book-vs-TR, etc.).
