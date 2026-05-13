# Book vs TSR reconciliation

Phase 5 of the post-TR intelligence roadmap. `opendqi emir book-reconcile`
compares a firm's **internal booking system export** against the
**TR Trade State Report** (`auth.107`) and surfaces every UTI-level
discrepancy.

This is *internal-book vs TR-state* reconciliation. It is distinct
from the counterparty pairing flow (`opendqi emir reconcile` against
`auth.106`) which compares the firm's submission to its
counterparty's.

```bash
opendqi emir book-reconcile \
  --book ./internal_book.csv \
  --tsr ./tr_state.xml \
  --mapping ./book_mapping.yml \
  --out ./book_vs_tsr_report/
```

Outputs:

- `summary.json` — regime-uniform scan summary.
- `book_vs_tsr_issues.csv` — flat list of every reconciliation
  defect, sorted deterministically.
- `book_vs_tsr_report.html` — HTML view of the same data.

## Catalog (7 checks)

| Check ID | Dimension | Severity | What it detects |
|---|---|---|---|
| `EMIR.BREC.IN_BOOK_NOT_IN_TSR` | Consistency | High | UTI is in the book but absent from the TSR — the firm thinks it reported the trade but the TR has no record. |
| `EMIR.BREC.IN_TSR_NOT_IN_BOOK` | Consistency | High | UTI is outstanding at the TR but absent from the book — the firm doesn't recognise an active trade the TR is tracking. |
| `EMIR.BREC.NOTIONAL_MISMATCH` | Accuracy | High | `notional_amount` differs between book and TSR. |
| `EMIR.BREC.NOTIONAL_CURRENCY_MISMATCH` | Validity | Warning | `notional_currency` differs (case-insensitive). |
| `EMIR.BREC.VALUATION_MISMATCH` | Accuracy | Warning | `valuation_amount` differs by more than 1% (relative tolerance). |
| `EMIR.BREC.MATURITY_MISMATCH` | Accuracy | High | `maturity_date` differs. |
| `EMIR.BREC.STATUS_MISMATCH` | Consistency | Warning | Book shows the trade as active (no `termination_date`) but the TSR reports it as `TERMINATED`. |

## Algorithm

```
1. Parse the YAML mapping (CsvMapping::from_path).
2. Load the book CSV via read_emir_csv → Vec<EmirRecord>.
3. Load the TSR XML via read_emir_tr_state_xml → Vec<TrStateRecord>.
4. Index both sides by UTI (trimmed, non-empty keys only).
5. Walk the union of UTIs:
   - book only             → IN_BOOK_NOT_IN_TSR
   - TSR only (outstanding) → IN_TSR_NOT_IN_BOOK
   - both                  → compare 5 fields, emit MISMATCH issues
6. Sort issues deterministically and write outputs.
```

The pure helper `compute_book_reconcile_issues(book, tsr)` is exposed
internally and unit-tested with one positive case per check_id plus a
clean-baseline negative case.

## Thresholds

- **Valuation tolerance** (1% relative) — compile-time constant in
  v1; will migrate to `Thresholds` config when a real customer
  profile emerges.

## Mapping file

The mapping format is the same one used by `opendqi emir scan` over
CSV. The book mapping only needs to declare the fields the
reconciliation actually compares:

```yaml
fields:
  uti: trade_uti
  notional_amount: notional
  notional_currency: notional_ccy
  maturity_date: maturity
  valuation_amount: valuation
  valuation_currency: valuation_ccy
  termination_date: terminated_on

date_format: "%Y-%m-%d"
datetime_format: "%Y-%m-%dT%H:%M:%S%.fZ"
```

Any fields not listed are simply left as `None` on each record and
are ignored by the reconciliation (they cannot cause false
positives).

## Out of scope (v1)

- **Parquet book input** — CSV only for now.
- **Multi-file books** — single CSV per run; trivial to extend.
- **`EMIR.BREC.COUNTERPARTY_MISMATCH`** — the book's LEI semantics
  vary across firms; reserved for a later milestone.
- **SFTR book-reconcile** — Phase 6.
