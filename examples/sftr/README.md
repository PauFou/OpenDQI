# SFTR fixtures

Synthetic, schema-shaped ISO 20022 XML fixtures for every SFTR
parser OpenDQI ships. All values are synthetic.

| Directory | ISO 20022 message | Use case | CLI command |
|---|---|---|---|
| `iso20022/`, `tier2.csv` | `auth.052.001.02` (SFTR TAR) | Firm submission scan | `opendqi sftr scan <file>` |
| `tr_activity/auth052-*.xml` | `auth.052.001.02` (TR replay of TAR) | TR-side activity scan | `opendqi sftr tr-activity-scan` |
| `tr_state/auth079-*.xml` | `auth.079.001.02` (SFTR TSR) | TR state snapshot | `opendqi sftr tr-state-scan` |
| `reconciliation/auth080-*.xml` | `auth.080.001.02` (Reconciliation Status Advice) | TR-side reconciliation | `opendqi sftr reconcile` |
| `missing_collateral/auth083-*.xml` | `auth.083.001.02` (Missing Collateral Request) | TR-side missing-collateral request (v0.17+) | `opendqi sftr missing-collateral` |
| `tr_state/auth079-*.xml` + `tr_activity/auth052-*.xml` | both | 2-layer audit | `opendqi sftr tr-audit` |
| `book_reconcile/` | CSV book + `auth.079` | Book ↔ TR reconciliation | `opendqi sftr book-reconcile` |
| `margin_activity/auth070-*.xml` | `auth.070.001.02` (Margin Activity) | Margin event stream (v0.18+) | `opendqi sftr mar-scan` (v0.20+) or `--mar` flag on `data-quality-pack` |
| `margin_state/auth085-*.xml` | `auth.085.001.02` (Margin State) | CCP-cleared portfolio margin state (v0.18+) | `opendqi sftr msr-scan` (v0.20+) or `--msr` flag |
| `tr_status_advice/auth084-*.xml` | `auth.084.001.02` (Transaction Status Advice) | Aggregate rejection statistics (v0.18+) | `opendqi sftr tr-status-advice-scan` (v0.20+) or `--tr-status-advice` flag |
| `reuse_activity/auth071-*.xml` | `auth.071.001.02` (Reused Collateral Activity) | Collateral reuse / reinvestment event log (v0.18+) | `opendqi sftr reuse-activity-scan` (v0.20+) or `--reuse-activity` flag |
| `reuse_state/auth086-*.xml` | `auth.086.001.02` (Reused Collateral State) | Reused collateral state snapshot (v0.18+) | `opendqi sftr reuse-state-scan` (v0.20+) or `--reuse-state` flag |
| various combined | multi-message | Full DQI Pack (9 layers) | `opendqi sftr data-quality-pack --tsr ... --mar ... --msr ... ...` |

**SFTR vs EMIR asymmetries:**

- No `auth.092`-equivalent for SFTR (rejection feedback). The ESMA
  did not publish an SFTR rejection-statistics message; the
  closest is `auth.080` (reconciliation status advice), already
  covered, but its semantics are reconciliation not rejection.
  The v0.18 `auth.084` Transaction Status Advice carries
  aggregate rejection counts and powers `DQI_REJ_RATE_SFTR` in
  the DQI pack.
- v0.20 brings full per-layer SFTR CLI standalone ergonomy
  (every layer above can now be scanned in isolation, mirror of
  EMIR's `mar-scan` / `msr-scan` / etc.).
- `conformance/` and `rejection_profile/` hold fixtures used by
  integration and XSD-conformance suites (not user-facing scan
  inputs).

For Python equivalents, see
[`../../docs/python.md`](../../docs/python.md) and the matching
[`../python/`](../python/) script kit. All v0.20 SFTR per-layer
CLI subcommands have matching `opendqi.sftr.*_scan` Python
wrappers shipped in v0.21.
