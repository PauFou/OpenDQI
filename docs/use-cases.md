# OpenDQI — Use cases

Three concrete scenarios that mirror the three workflows on the
README. Each one shows the question, the input, the command, and
what you actually get back. The counts below come from the
synthetic fixtures shipped in [`examples/quickstart-emir/`](../examples/quickstart-emir/)
— the exact same files the [`scripts/demo.sh`](../scripts/demo.sh)
runner uses, and the same byte-for-byte outputs the golden suite
pins in CI.

The 3 layers behind these scenarios (TAR / TSR / Rejection) are
explained in [`positioning.md`](positioning.md); this document is
the operator-facing complement — *what you do with them on a
Tuesday morning*.

---

## 1. TR state health — "what does the TR actually think I have open?"

### The question

> Friday EOD, the daily TR state report drops. 1 500 derivatives
> show up as outstanding at the TR. Internally I think I have
> ~1 200 active trades. **Where's the noise?** Stale valuations?
> Trades I've already terminated but the TR hasn't moved? Duplicate
> active UTIs?

### The input

A single ISO 20022 `auth.107` Trade State Report from the TR.

### The command

```bash
opendqi emir tr-state-scan \
  examples/quickstart-emir/auth107-tsr.xml \
  --out ./report/
```

### What you get

3 deterministic artefacts in `./report/` :

- `tr_state_report.html` — the human-facing executive report
- `tr_state_issues.csv` — one row per issue, with structured
  `evidence_json` you can join into a ticketing tool
- `summary.json` — counts machine-readable for dashboards

On the shipped 8-record fixture: **16 issues, quality score 86.6**,
broken down as :

| Severity | Count | What it surfaces here |
|---|---:|---|
| critical | 2 | `EMIR.TST.DUPLICATE_ACTIVE_UTI` — same UTI counted twice as outstanding |
| high     | 5 | `EMIR.TST.ACTIVE_PAST_MATURITY` (1) · `EMIR.TST.MISSING_VALUATION` (1) · `EMIR.TST.STALE_VALUATION` (2) · `EMIR.TST.VALUATION_AFTER_TERMINATION` (1) |
| warning  | 1 | `EMIR.TST.PLACEHOLDER_MATURITY` — `9999-12-31` in `maturity_date` |
| info     | 8 | `EMIR.TST.OUTSTANDING_SUMMARY` — one per outstanding trade (the operational baseline) |

Each `high`/`critical` issue carries a `record_id` you can trace
back into the auth.107 XML line, the offending field name and
value, and a one-line human message. The `EvidenceItem` block is
visible in the HTML report and as the `evidence_json` column of
the CSV — that's the audit trail.

### What you don't get

This scan does **not** look at counterparty / TR pairing (use
`emir recon-stats` for that, fed by `auth.091`), nor at margin
state (`emir msr-scan` over `auth.109`). The TSR layer is
intentionally narrow — *snapshot health, one report*.

---

## 2. Rejection intelligence — "what's the TR throwing back, and why?"

### The question

> 47 reports rejected by the TR this week. **Are these one-off
> mistakes or a recurring pattern?** Which validation rules trip
> us most? Are the same UTIs being rejected again and again?

### The input

An ISO 20022 `auth.092` Derivatives Trade **Rejection** Statistical
Report from the TR (one report per submission batch).

### The commands

Single-file analytics — for a one-shot view of a single rejection
batch :

```bash
opendqi emir feedback \
  examples/quickstart-emir/auth092-feedback.xml \
  --store ./history.db \
  --out ./feedback/
```

(`--store` is required — feedback rows are persisted into the local
SQLite history store so the operational `feedback list / resolve /
stale` workflow can track them through to closure.)

On the shipped 2-record fixture: **2 critical issues, quality
score 75.0** — both `EMIR.FBK.TR_REJECTED_UTI`, with the full list
of `DtldVldtnRule` codes preserved (`VR-0001`, `VR-0042`, `VR-0100`)
in the issue message and as structured `evidence_json` — so the
top-rules question reduces to a `cut`/`sort`/`uniq` over the
`evidence_json` column.

Going further — the **rejection profile loop** turns this
intelligence into pre-submission gates. With the optional
`--store ./history.db` the feedback rows are persisted, and over
time you build a `rejection_profile.yml` :

```bash
opendqi feedback list --store ./history.db --regime emir
opendqi feedback analytics --store ./history.db --regime emir \
  --out ./rejection_profile.yml
```

Then on your *next* submission, the same profile flags records
likely to be rejected before they even reach the TR :

```bash
opendqi emir scan \
  my-submission.csv --mapping mapping.yml \
  --rejection-profile ./rejection_profile.yml \
  --out ./pre-submission/
```

That fires the `EMIR.PSC.*` family — *Pre-Submission Confidence* —
which is the post-TR ↔ pre-TR loop the product is built around.

### What you don't get

`auth.092` is a *statistical* report (it's a list of rejected UTIs,
with rule codes — not a generic root-cause analyser). The
"`why was rule X violated for UTI Y?`" question still requires
reading the underlying submission. OpenDQI gives you the *list*
and the *counts*; deep field-level diagnostic is a manual step
(or `book-reconcile`'s field mismatch ground truth).

---

## 3. Combined audit — "give me one operational report for the committee"

### The question

> Monday morning, ops committee. **One artefact**: this week's TR
> activity, the current TR state, and the rejections — cross-
> referenced so we don't have to flip between 3 reports to spot
> that the trade rejected on Tuesday is now ghosted as outstanding
> on Friday's TSR.

### The input

Three files together — the week's TAR (`auth.030`), the latest TSR
(`auth.107`) and the rejection feedback (`auth.092`).

### The command

```bash
opendqi emir tr-audit \
  --tar     examples/quickstart-emir/auth030-tar.xml \
  --tsr     examples/quickstart-emir/auth107-tsr.xml \
  --feedback examples/quickstart-emir/auth092-feedback.xml \
  --out ./audit/
```

### What you get

A **single consolidated** `tr_audit_report.html` covering all three
layers — TAR activity (`EMIR.TRA.*`), TSR state (`EMIR.TST.*`),
rejections (`EMIR.FBK.*`) — *plus* 3 cross-layer coherence checks
(`EMIR.AUD.*`) that only this command can run :

- `EMIR.AUD.REJECTED_BUT_OUTSTANDING_IN_TSR` — a UTI the TR
  rejected this week is showing up as outstanding in this TSR
- `EMIR.AUD.MODI_WITHOUT_PRIOR_NEWT_IN_TSR` — modification without
  a prior NEWT in TSR state
- `EMIR.AUD.TERM_BUT_STILL_OUTSTANDING_IN_TSR` — TERM in this TAR
  but still outstanding in this TSR

On the shipped 3-file fixture (20 records total): **251 issues,
quality score 4.35** — broken down 11 critical / 134 high / 98
warning / 8 info, across 5 dimensions (completeness 189 / consistency 43 /
uniqueness 9 / accuracy 8 / validity 2). It's deliberately a
noisy fixture — *that's the demo*: the audit report makes triage
across layers tractable.

### What you don't get

`tr-audit` is multi-file but **single-batch** (the 3 files passed
in one invocation). For *cross-batch* analyses (compare today's
TSR to last week's; track a rejection's UTI through 4 weeks of
TARs), use `--store ./history.db` on each individual layer
command and inspect the SQLite store via the
[`feedback`](feedback-checks.md) /
[`lifecycle-cross-batch`](lifecycle-cross-batch.md) workflows.

---

## When to use which

| Cadence | Workflow | Command |
|---|---|---|
| **Daily** (post-TR drop) | TR state health | `emir tr-state-scan` |
| **Weekly** (rejection triage) | Rejection intelligence | `emir feedback` → `feedback analytics` → `rejection_profile.yml` → next `emir scan --rejection-profile` |
| **Monthly / on-demand** (committee, audit) | Combined audit | `emir tr-audit` |
| **Pre-submission** (gate) | Profile-driven pre-flight | `emir scan --rejection-profile ...` |

Beyond these three, OpenDQI also ships dedicated subcommands for
[margin state](emir-mar-msr.md) (`mar-scan` / `msr-scan`),
[reconciliation statistics](emir-recon-stats.md) (`recon-stats`),
[TR data-quality warnings](emir-warnings.md) (`warnings`),
[collateral cross-reference](collateral-audit.md)
(`collateral-audit`), and [book-vs-TR reconciliation](book-reconcile.md)
(`book-reconcile`). They share the same 3-layer mental model — pick
the command that matches the question, not the file.

## SFTR

The same three scenarios apply to SFTR — the commands are
`opendqi sftr {tr-state-scan, missing-collateral, tr-audit}` over
`auth.079` / `auth.083` / `auth.052+079`. SFTR has **no rejection-
feedback message** (the synthetic `SFTR.FBK.*` layer was retired
in Milestone 0.4); the rejection-intelligence workflow above is
EMIR-only.
