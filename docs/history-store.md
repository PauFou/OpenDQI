# History store

OpenDQI's history store is a local SQLite file that persists scanned
EMIR / SFTR records so that lifecycle checks have access to the
records of prior scans. See [`lifecycle-checks.md`](lifecycle-checks.md)
for the catalog of checks that consume the store.

The store is **opt-in**. Without `--store <path>`, OpenDQI runs
entirely in memory; no SQLite file is opened or created.

## Usage

```bash
# Enable persistence + lifecycle checks for an EMIR scan
opendqi emir scan ./reports/april/ \
    --mapping ./mapping.yml \
    --store ./opendqi-history.db \
    --out ./report/

# Same for SFTR
opendqi sftr scan ./sftr/april/ \
    --store ./opendqi-history.db \
    --out ./report/
```

The first run on a fresh store materialises the schema and persists
the batch — only `_WITHOUT_NEWT` checks can fire (the store has no
prior records yet). Subsequent runs see the accumulated history and
will start raising `DUPLICATE_NEWT_FOR_UTI`, `VALUATION_REGRESSION`,
and `VALUATION_AFTER_TERMINATION` issues as appropriate.

## Schema (v1)

Idempotent `CREATE TABLE IF NOT EXISTS` statements run on every
`open_store` call.

```sql
scans(scan_id, regime, started_at, file_count, record_count)
emir_records(scan_id, record_id, source_file, uti, prior_uti,
             action_type, event_type,
             execution_timestamp, event_timestamp, reporting_timestamp,
             effective_date, maturity_date, termination_date,
             valuation_amount, valuation_timestamp, valuation_currency,
             ingested_at)
sftr_records(scan_id, record_id, source_file, uti, prior_uti,
             action_type, event_type,
             execution_timestamp, event_timestamp, reporting_timestamp,
             effective_date, maturity_date, termination_date,
             settlement_date, sft_type,
             ingested_at)
```

Indexes on `uti` and `(uti, action_type)` keep lifecycle lookups
fast.

Conventions:

- Timestamps are stored as Unix-second `INTEGER`.
- Dates are stored as ISO `TEXT` (`YYYY-MM-DD`) for readability.
- Decimal amounts are stored as exact `TEXT` to preserve full ESMA
  precision (a `REAL` column would lose decimal digits beyond IEEE
  754).
- Each scan inserts a fresh row per record — there is no upsert by
  UTI. Keeping every version is what lets the lifecycle checks reason
  about actions over time.

## Where the file lives

OpenDQI does not pick a default path. Whatever you pass to `--store`
is what you get; parent directories are created if missing.

Recommended locations:

- Personal workflow: `~/.local/share/opendqi/history.db`.
- Team / shared workflow: a path on a controlled file system, with
  appropriate access policies. SQLite supports a single writer at a
  time — concurrent scans on the same store should be serialised.

## Privacy and data hygiene

- The store contains exactly the fields listed in the schema above
  (UTIs, action types, timestamps, valuations, dates, SFT type). It
  does **not** persist the entire canonical record, raw XML, or full
  counterparty / portfolio detail.
- Treat the file as confidential: it is your scanned history.
- The OpenDQI repository's `.gitignore` already excludes `*.db`,
  `*.sqlite`, and their journal companions to make accidental commits
  unlikely.

## Operations

- **Backup**: copy the file. SQLite databases are single-file.
- **Reset**: delete the file. The next `opendqi … --store <path>`
  invocation re-creates an empty store.
- **Inspect**: any SQLite client works (`sqlite3 ./history.db`).
- **Compact**: `sqlite3 ./history.db 'VACUUM;'` if it grows large.

## Limitations (v1)

- No retention / TTL policy. Old scans accumulate until you delete or
  vacuum.
- No `opendqi history` subcommand yet (list / diff / purge). Planned.
- No schema versioning table. The v1 layout is intentionally narrow
  and is expected to stay stable until a real breaking change is
  needed.
- Single-process write. SQLite handles its own file locking but the
  CLI does not attempt to coordinate multiple concurrent writers.
