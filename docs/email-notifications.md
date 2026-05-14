# Email notifications

`opendqi emir scan --email-config smtp.yml ./submissions.csv …`
emails the scan report (HTML inline + `summary.json` + `issues.csv`
attachments) to a configured list of recipients after the scan
finishes. CLAUDE.md priority #8 — opt-in, never blocking the scan
itself.

## Quick start

1. Copy the env-var template and set your SMTP password:

   ```bash
   cp .env.example .env
   # edit .env, set OPENDQI_SMTP_PASS=<app-specific-password>
   ```

2. Write an `smtp.yml` config:

   ```yaml
   enabled: true
   host: smtp.gmail.com
   port: 587
   use_tls: true
   username: alerts@example.com
   password_env: OPENDQI_SMTP_PASS
   from: "OpenDQI <alerts@example.com>"
   to:
     - compliance@example.com
     - reporting-ops@example.com
   subject_template: "OpenDQI {regime} — {critical} crit / {high} high"
   ```

3. Run a scan:

   ```bash
   export $(grep -v '^#' .env | xargs)
   opendqi emir scan ./examples/emir/sample.csv \
     --mapping examples/emir/sample_mapping.yml \
     --email-config ./smtp.yml \
     --out ./report/
   ```

The scan writes `report.html` / `summary.json` / `issues.csv` to
`./report/` *and* emails them. Setting `enabled: false` in the YAML
makes the email step a no-op (useful for staging).

## Config schema (`SmtpConfig`)

| Field | Required | Default | Description |
|---|---|---|---|
| `enabled` | no | `true` | When `false`, `send_report_email` returns `Ok(false)` without contacting the SMTP server. |
| `host` | yes | — | SMTP host (e.g. `smtp.gmail.com`). |
| `port` | yes | — | SMTP port. 587 = STARTTLS; 465 = SMTPS (set `use_tls: true`). |
| `use_tls` | no | `true` | Upgrade via STARTTLS. Set `false` only for plain local relays. |
| `username` | yes | — | SMTP username. |
| `password_env` | no | `OPENDQI_SMTP_PASS` | Name of the env var holding the password. Never store the password in YAML. |
| `from` | yes | — | RFC 5322 `From:` (e.g. `Name <addr@host>`). |
| `to` | yes | — | List of recipient addresses. |
| `subject_template` | no | `"OpenDQI scan {regime} — {critical} critical / {high} high (score {score:.1})"` | Supports placeholders `{regime}`, `{records}`, `{critical}`, `{high}`, `{score}` / `{score:.1}`. |

## Security

- **Never commit the populated `.env`** — `.gitignore` excludes `.env` and `.env.*` by default (only `.env.example` is whitelisted via the `!.env.example` rule).
- The SMTP password is read **only** from the environment, never from YAML. Even if your config file is accidentally committed, no credentials leak.
- The transport uses `lettre` with `rustls-tls` (no OpenSSL link). TLS verification is enabled by default.
- Localhost-only test transports for non-prod relays: set `use_tls: false`, `host: 127.0.0.1`, `port: 25` and skip authentication by leaving `password_env` pointing at an empty variable (the unset case fails fast).

## CLI flag

`--email-config <path>` is currently wired into:

- `opendqi emir scan`
- `opendqi sftr scan`
- `opendqi emir tr-state-scan` (TSR daily snapshot — most-emailed report in ops practice)
- `opendqi emir tr-audit` (consolidated TAR + TSR + feedback report)

Add it to the remaining commands (`emir tr-activity-scan`, `emir
mar-scan`, `emir msr-scan`, `emir feedback`, `emir recon-stats`,
`emir book-reconcile`, and their SFTR counterparts) by mirroring the
same 3-line pattern (`SmtpConfig::from_yaml_file` →
`send_report_email`) after the report-write block.

## Tests

`crates/opendqi-report/src/email.rs::tests` covers:

- subject template placeholder substitution
- `enabled: false` → no-op
- YAML round-trip with defaults

Live SMTP is **not** exercised in CI — integration with a real relay
is left to the operator. To smoke-test locally, point `host`/`port`
at a `python3 -m smtpd -n -c DebuggingServer localhost:1025`
listener.
