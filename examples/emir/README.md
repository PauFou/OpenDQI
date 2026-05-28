# EMIR fixtures

Synthetic, schema-shaped ISO 20022 XML fixtures for every EMIR
parser OpenDQI ships. All values are synthetic — no real
counterparties, UTIs, or notionals.

| Directory | ISO 20022 message | Use case | CLI command |
|---|---|---|---|
| `iso20022/`, `sample.xml`, `sample.csv` | `auth.030.001.03` (TAR) | Firm submission scan | `opendqi emir scan <file>` |
| `tr_activity/auth030-*.xml` | `auth.030.001.03` (TR replay of TAR) | TR-side activity scan | `opendqi emir tr-activity-scan` |
| `tr_state/auth107-*.xml` | `auth.107.001.01` (TSR) | TR state snapshot | `opendqi emir tr-state-scan` |
| `feedback/auth092-*.xml` | `auth.092.001.04` (Rejection feedback) | Pre-submission rejection profile | `opendqi emir feedback` (CLI), `opendqi.emir.feedback` (Python) |
| `feedback/auth092-*.xml` | + `tr_state/` + `tr_activity/` | 3-layer audit | `opendqi emir tr-audit` |
| `collateral_audit/` | `auth.107` + `auth.109` cross-message | Article 11 collateral cross-ref | `opendqi emir collateral-audit` |
| `book_reconcile/` | CSV book + `auth.107` | Book ↔ TR reconciliation | `opendqi emir book-reconcile` |
| `mar/auth108-*.xml` | `auth.108.001.01` (Margin Activity) | Margin event stream | `opendqi emir mar-scan` |
| `msr/auth109-*.xml` | `auth.109.001.01` (Margin State) | Margin state snapshot | `opendqi emir msr-scan` |
| `recon_stats/auth091-*.xml` | `auth.091.001.02` (Reconciliation Stats) | TR-side reconciliation | `opendqi emir recon-stats` |
| `warnings/auth106-*.xml` | `auth.106.001.01` (Warnings) | TR warnings report | `opendqi emir warnings` |
| `positions/auth090-*.xml` | `auth.090.001.02` (Position Set) | Aggregated position exposures (v0.18+) | `opendqi emir data-quality-pack --positions` |
| `query/auth029-*.xml` | `auth.029.001.04` (Trade Report Query) | Firm-side query envelope (v0.20+) | `opendqi emir query-scan` |
| `status_advice/auth031-*.xml` | `auth.031.001.01` (Status Advice ack) | TR -> firm ack envelope (v0.20+) | `opendqi emir status-advice-scan` |

**Honest scope notes:**

- The table above lists **user-facing scan inputs only**. The
  following subdirectories also exist in `examples/emir/` but are
  test-suite-only (used by integration / XSD-conformance /
  pre-submission rejection harnesses) and are **not documented
  here** because they are not meant to be invoked directly:
  `broken/`, `conformance/`, `iso20022/` (sample-only),
  `rejection_profile/`, `risk_mitigation/`, `schemas/`,
  plus the loose CSV / YAML files (`extended-checks.yml`,
  `margin-and-enums.{csv,yml}`, `sample.{csv,xml}`,
  `sample_mapping.yml`, `tier2.{csv,yml}`,
  `violates-schema.xml`).
- `query/` and `status_advice/` are **envelope-only** messages (no
  derivatives payload). They fire one sanity check each
  (`EMIR.QRY.ENVELOPE_WELLFORMED` / `EMIR.ACK.ENVELOPE_WELLFORMED`)
  verifying minimum identity fields. See
  [`../../docs/auth-messages/emir-auth029.md`](../../docs/auth-messages/emir-auth029.md)
  and [`emir-auth031.md`](../../docs/auth-messages/emir-auth031.md)
  for the rationale.
- `auth.078` does **not exist** in the ESMA bundle (the "11 EMIR
  messages" tally elsewhere was based on a conjectured message
  name). See [`../../docs/auth-messages/emir-auth078.md`](../../docs/auth-messages/emir-auth078.md)
  for the full investigation.

For a fast end-to-end walk-through of the 3 primary EMIR
workflows, run [`../../scripts/demo.sh`](../../scripts/demo.sh).

For Python equivalents, see
[`../../docs/python.md`](../../docs/python.md) and the matching
[`../python/`](../python/) script kit.
