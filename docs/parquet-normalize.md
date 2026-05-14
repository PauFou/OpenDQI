# Parquet normalize — `opendqi {emir,sftr} normalize`

OpenDQI can emit a canonical Parquet representation of the ingested
EMIR / SFTR records, ready for downstream analytics (DuckDB, Polars,
PyArrow, Apache Spark, …).

```bash
opendqi emir normalize examples/emir/sample.csv \
  --mapping examples/emir/sample_mapping.yml \
  --out /path/to/emir.parquet

opendqi sftr normalize examples/sftr/tier2.csv \
  --mapping examples/sftr/tier2.yml \
  --out /path/to/sftr.parquet
```

Inputs may be XML or CSV (single file or directory). `--mapping` is
required when at least one CSV file is in the input set, optional
otherwise. Output is a **single Parquet file**, not a directory.

## Format

- **Compression**: Snappy (good size + fast read in every downstream
  tool).
- **Encoding**: standard Apache Arrow 53.x Parquet writer.
- **Decimal columns**: `Decimal128(38, 10)` — 28 integer digits + 10
  fractional digits. Re-scaled losslessly from the source
  `rust_decimal::Decimal`. Covers regulatory notionals up to 10^28
  and haircuts at 1e-10 precision.
- **Timestamps**: `Timestamp(Microsecond, "UTC")`. All times are
  stored in UTC; analytics tools may project to a local zone.
- **Dates**: `Date32` — days since 1970-01-01.
- **Booleans**: `Boolean` nullable.
- **Strings**: `Utf8` nullable.
- **`raw_fields`**: serialized as a JSON object string (BTreeMap →
  alphabetical key order, deterministic). Use `json_extract` / `->`
  in DuckDB or `pl.col(...).str.json_path_match(...)` in Polars.
- **`regime` column**: literal `"EMIR"` or `"SFTR"`. Convenience for
  unioning the two files downstream.

## EMIR schema (54 columns)

Identifiers : `source_file, record_id, regime, uti, prior_uti,
action_type, event_type, entity_responsible_for_reporting,
counterparty_1, counterparty_2`.

Product / leg-1 : `asset_class, product_id, underlying_id,
notional_amount, notional_currency, price, price_currency`.

Timestamps : `execution_timestamp, event_timestamp,
reporting_timestamp, effective_date, maturity_date,
termination_date`.

Valuation : `valuation_amount, valuation_currency,
valuation_timestamp`.

Margins : `initial_margin_posted, initial_margin_collected,
variation_margin_posted, variation_margin_collected,
collateral_portfolio_code, clearing_status,
collateralisation_category`.

Leg-2 : `leg2_notional_amount, leg2_notional_currency,
leg1_payment_frequency, leg2_payment_frequency`.

Clearing / nature / agreement / governance : `clearing_ccp_lei,
intragroup_indicator, hedging_indicator, valuation_type,
trading_capacity, commercial_or_treasury_financing,
reporting_obligation_indicator, corporate_sector, nature,
master_agreement_type, master_agreement_version,
confirmation_method`.

Greeks / MTM : `mtm_value_change, delta, gamma, vega`.

Catch-all : `raw_fields` (JSON).

## SFTR schema (31 columns)

Identifiers : `source_file, record_id, regime, uti, prior_uti,
action_type, event_type, entity_responsible_for_reporting,
counterparty_1, counterparty_2, sft_type`.

Master agreement : `master_agreement_type, master_agreement_version`.

Loan / collateral : `loan_value, loan_currency, collateral_value,
collateral_currency, haircut, reuse_indicator, rebate_rate,
lending_fee`.

Timestamps : `execution_timestamp, event_timestamp,
reporting_timestamp, effective_date, maturity_date,
termination_date, settlement_date`.

Settlement / collateral details : `collateral_portfolio_code,
collateral_isin`.

Catch-all : `raw_fields` (JSON).

## Downstream usage

### DuckDB

```sql
-- Top counterparties by notional, IR only.
SELECT counterparty_1, SUM(notional_amount) AS total_notional
FROM '/path/to/emir.parquet'
WHERE asset_class = 'IR'
GROUP BY counterparty_1
ORDER BY total_notional DESC
LIMIT 10;

-- Inspect raw_fields JSON.
SELECT uti, raw_fields::JSON->>'OptnTp' AS option_type
FROM '/path/to/emir.parquet'
WHERE raw_fields IS NOT NULL;
```

### Polars (Rust / Python)

```python
import polars as pl

df = pl.read_parquet("/path/to/emir.parquet")
print(df.schema)
print(df.filter(pl.col("regime") == "EMIR").group_by("asset_class").count())
```

### PyArrow

```python
import pyarrow.parquet as pq

table = pq.read_table("/path/to/sftr.parquet")
print(table.num_rows, table.num_columns)
print(table.schema)
```

## Compatibility

Tested with:
- Apache Arrow / Parquet 53.4 (Rust ecosystem).
- DuckDB ≥ 0.10 (Decimal128, JSON, Snappy).
- Polars ≥ 0.40, PyArrow ≥ 14.

## Reading Parquet back (Phase 8.7)

`opendqi {emir,sftr} scan` accepts `.parquet` inputs alongside XML
and CSV. The round-trip is complete:

```bash
# Normalize one source, then scan the Parquet via the same checks
opendqi emir normalize examples/emir/sample.csv \
  --mapping examples/emir/sample_mapping.yml \
  --out /tmp/emir.parquet

opendqi emir scan /tmp/emir.parquet --out /tmp/scan-from-parquet/
```

Schema tolerance:

- **Missing columns** — the reader looks columns up by name. Any
  field absent from the file stays at `Default` / `None` on the
  reconstructed record. Forward-compat with producers that emit a
  subset of the canonical schema.
- **Extra unknown columns** — ignored.
- **`regime` mismatch** — if the file contains a `regime` column
  with a value incompatible with the runner (e.g. `read_emir_parquet`
  on a `regime=SFTR` file), the read returns a clear error.
- **Decimal scale** — values written at scale 10 are normalised on
  read (trailing zeros stripped), so checks that key on
  `Decimal::scale()` (e.g. `EMIR.VLD.NOTIONAL_PRECISION_BY_CURRENCY`)
  behave identically whether the input came from CSV or Parquet.

The identity:

```
scan(csv)  ≡  normalize(csv) -> scan(parquet)
```

holds modulo the `source_file` column (which naturally differs).

## Hors-scope (v1)

- Compression alternative (Zstd, LZ4) — Snappy uniquement v1.
- Row-group customisation (taille, statistiques avancées).
- Chunking / streaming — single RecordBatch par fichier, suffisant
  pour des batches OpenDQI typiques (< 100k records).
- Schema versionning : si le schéma évolue, un nouveau milestone
  ajoutera une colonne `_opendqi_schema_version` ou utilisera la
  metadata Parquet pour le marquer.
- Lecture Parquet pour les sous-commandes hors `scan` (feedback,
  reconcile, tr-state-scan, mar-scan, …) : `scan` est le seul
  use-case clair v1.
