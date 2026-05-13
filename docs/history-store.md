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

## Schema

Idempotent `CREATE TABLE IF NOT EXISTS` statements run on every
`open_store` call. The schema is additive — new tables are added in
later releases without breaking older databases.

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

feedbacks(feedback_id, scan_id NULL, regime, uti,
          feedback_type, reason_code, reason_description, reported_field,
          source_file, feedback_timestamp,
          status DEFAULT 'open' CHECK status IN ('open','resolved','stale'),
          status_set_at, ingested_at)
```

Indexes on `uti`, `(uti, action_type)`, `feedbacks(uti, status)`, and
`feedbacks(status)` keep lifecycle and workflow lookups fast.

## Feedbacks table — workflow

The `feedbacks` table persists every row ingested by
`opendqi {emir,sftr} feedback`. Each row starts in `status='open'`
and can be transitioned to `resolved` or `stale` via the top-level
`opendqi feedback` subcommand:

```bash
# List
opendqi feedback list --store ./history.db
opendqi feedback list --store ./history.db --regime emir --status open
opendqi feedback list --store ./history.db --uti UTI-A

# Mark resolved (e.g. after re-submitting the corrected report)
opendqi feedback resolve --store ./history.db --uti UTI-A

# Mark stale (no longer relevant — TR moved on, do not surface again)
opendqi feedback stale --store ./history.db --uti UTI-A
```

Transition rules for v1: `open → resolved` or `open → stale`. There
is no automatic `resolved → open` transition; a fresh ingestion of
the same UTI from a new feedback file inserts a new row in
`status='open'`. Idempotent: re-marking a row to its current status
is a no-op (`updated = 0`).

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
