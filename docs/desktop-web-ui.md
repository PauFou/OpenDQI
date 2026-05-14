# OpenDQI desktop — local web UI

`opendqi desktop` launches a tiny axum-based web UI on
`http://127.0.0.1:7878` so users can drag an EMIR / SFTR file into a
browser, run a scan, and download the resulting reports. No data
leaves the machine; binds to localhost only.

```bash
opendqi desktop                 # binds 127.0.0.1:7878
opendqi desktop --port 8080     # custom port
```

The browser opens to a minimal upload form:

- **Operation** — pick which scan to run:
  - *Standard scan* (default) — submission XML/Parquet.
  - *TR Trade State Report* — auth.107 (EMIR) / auth.079 (SFTR).
  - *TR Trade Activity Report* — auth.030 (EMIR) / auth.052 (SFTR) replay.
  - *TR Feedback* — auth.092 (EMIR) / auth.080 (SFTR). v1 runs the
    format / namespace checks only; the store-cross-referenced
    `EMIR.FBK.*` / `SFTR.FBK.*` checks remain CLI-only.
  - *Reconciliation Statistics* — auth.091 (EMIR only).
  - *Margin Activity Report* — auth.108 (EMIR only).
  - *Margin State Report* — auth.109 (EMIR only).
  - *Validate* — XML well-formedness only. XSD schema validation
    remains CLI-only (`opendqi {emir,sftr} validate --xsd <path>`).
- **Regime** — EMIR or SFTR.
- **File** — drop or pick an `.xml` or `.parquet` file.
- **Run** — submits to `POST /api/scan` carrying the chosen operation
  in the multipart form.

On submit the server saves the upload to a per-process temp dir,
runs `default_checks()` (EMIR) or `default_sftr_checks()` (SFTR),
writes the standard report trio (`summary.json`, `issues.csv`,
`report.html`), then redirects to `/scans/{id}` where the user gets
a small dashboard with download links for each artifact.

## Supported inputs

v1 web UI supports **XML** and **Parquet** only — both are
self-describing. CSV inputs require a YAML mapping (regime-specific)
which is awkward to drop into a form, so CSV stays CLI-only for v1.

For CSV scans, use:

```bash
opendqi emir scan path/to/file.csv --mapping path/to/mapping.yml --out ./report/
```

## Routes

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/` | Upload form (HTML, minijinja-rendered). |
| `POST` | `/api/scan` | Multipart upload: `regime` (form field) + `file`. Runs the scan in a tokio `spawn_blocking` task, returns `303 See Other` → `/scans/{id}`. |
| `GET` | `/scans/{id}` | Results page: summary stats + download links. |
| `GET` | `/scans/{id}/{file}` | Streams a report artifact with `Content-Disposition: attachment`. |

## Storage and cleanup

Each `opendqi desktop` process gets a fresh temp directory:

```
$TMPDIR/opendqi-desktop-{pid}-{uuid}/
├── {scan-uuid-1}/
│   ├── <uploaded file>
│   ├── scan_meta.json
│   ├── summary.json
│   ├── issues.csv
│   └── report.html
└── {scan-uuid-2}/
    └── …
```

Cleanup is opt-out: the OS reclaims `$TMPDIR` on reboot. For
production-grade hygiene, run with a custom `TMPDIR` (e.g. a
RAM disk) and `rm -rf` after stopping the server.

## Security

- **Localhost binding only** (`127.0.0.1`). Remote access is not
  exposed; tunnel via SSH or reverse-proxy if needed.
- **Path traversal hardened**: scan ids and file names are
  alphanumeric / dotless / no slashes — `AppState::safe_join` rejects
  anything else.
- **No auth, no sessions, no cookies** — single-user local tool.
- **No CSRF protection** — same rationale; if you change this, also
  add CSRF tokens to the form.

## Architecture

- `opendqi-server` (new crate library + binary): axum router, three
  handlers, three minijinja templates, scan orchestration.
- `opendqi-cli::commands::desktop` calls `opendqi_server::serve(port)`
  on a multi-threaded tokio runtime built in-thread.
- The server depends on `opendqi-core`, `opendqi-io`, `opendqi-xml`,
  `opendqi-report` — same crate graph as the CLI runners.
- Scan logic in `crates/opendqi-server/src/scan.rs` is a stripped
  version of `opendqi-cli::commands::emir::run_scan`: no XSD, no
  store, no config thresholds (uses defaults). Future polish can
  bring those flags into the form.

## Testing

```bash
cargo test -p opendqi-server
```

Integration tests cover:

- `GET /` renders the form with the expected fields.
- `POST /api/scan` with an EMIR XML fixture redirects to a results
  page that lists `report.html` / `summary.json` / `issues.csv`.
- The `issues.csv` artifact streams with the right content type.
- Invalid `regime` is `400 Bad Request`.
- Unknown scan id returns `404 Not Found`; path traversal is
  rejected.

## Roadmap

- v1.1: CSV upload + paired YAML mapping field.
- v1.2: XSD validation toggle (the CLI's `--xsd <path>` flag).
- v1.3: drag-and-drop UX polish (the form already accepts drag-and-
  drop via the native `<input type=file>` element on macOS/Chrome).
- v1.4: history / list of recent scans on `/` with cleanup button.
- v2: SQLite history-store integration mirroring `--store`.
