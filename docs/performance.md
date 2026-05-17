# Performance — parallelism + benchmarks

OpenDQI's check loop fans out over the dimension `checks`, not
`records`. Records are small structs (40-50 typed fields); the
catalog is 213 checks (148 EMIR + 65 SFTR) running mostly O(n) over
records. Parallelizing the **checks** dimension is the high-leverage
choice: scheduling overhead amortises across `n_records` iterations
of each check.

## Implementation

All 20 `run_all*` runners in `crates/opendqi-core/src/dq/mod.rs` use
`rayon::par_iter().flat_map_iter(...)` :

```rust
let mut issues: Vec<DqIssue> = checks
    .par_iter()
    .flat_map_iter(|c| c.run(records, ctx))
    .collect();
finalize_issues(&mut issues, ctx); // severity overrides + sort
```

- `par_iter` distributes the check evaluations across the rayon
  global thread pool (defaults to one worker per logical core).
- `flat_map_iter` accepts each check's `Vec<DqIssue>` as a plain
  iterator, avoiding the cost of turning each tiny sub-Vec into a
  `ParallelIterator`.
- The final `finalize_issues` (severity overrides + sort) is
  sequential and runs in O(n log n) over the merged issue list —
  typically a few thousand items max, dwarfed by the check loop.

Every check trait (`Check`, `SftrCheck`, `LifecycleCheck`,
`TrStateCheck`, `MarginActivityCheck`, …) already requires
`Send + Sync`, so the migration to parallel execution is type-safe
at the trait level — adding a non-`Send + Sync` check would refuse
to compile.

## Running the benchmarks

Criterion is set up as a dev-dependency on `opendqi-core`:

```bash
cargo bench -p opendqi-core --bench check_loop
```

Two suites are exercised at 1k, 10k, 100k, and **1M** synthetic
records each (the 1M point is the "millions of records" scale data
point; Criterion auto-reduces the sample size for the slow case):

- `run_all_emir/{1000,10000,100000,1000000}` — `default_checks()`
  (the full 148-check EMIR single-batch catalog).
- `run_all_sftr/{1000,10000,100000,1000000}` — `default_sftr_checks()`
  (the 65-check SFTR catalog).

The synthetic generators are **deterministic** (index-driven, no
RNG) and populate ~30 typed fields per record so the vast majority
of checks evaluate non-trivially rather than short-circuiting on
absent data. Criterion handles warm-up, calibration, and outlier
detection.

## Reference numbers

Local-machine numbers (Apple Silicon, Rust stable, release build,
`lto=thin`), measured 2026-05-15 against the full 200-check
catalog with the enriched deterministic generators. They scale
roughly linearly with `n_records` once warm. **Indicative, not a
benchmark contract** — re-run `cargo bench` on your hardware.

| Workload | Records | Wall time | Throughput |
|---|---|---|---|
| `run_all_emir` | 1 000 | ~2.4 ms | ~411 k records/s |
| `run_all_emir` | 10 000 | ~23.4 ms | ~428 k records/s |
| `run_all_emir` | 100 000 | ~320 ms | ~312 k records/s |
| `run_all_emir` | 1 000 000 | ~13.7 s † | ~73 k records/s † |
| `run_all_sftr` | 1 000 | ~1.5 ms | ~684 k records/s |
| `run_all_sftr` | 10 000 | ~12.8 ms | ~779 k records/s |
| `run_all_sftr` | 100 000 | ~164 ms | ~609 k records/s |
| `run_all_sftr` | 1 000 000 | ~3.0 s † | ~335 k records/s † |

† The 1M rows were measured 2026-05-17 against the **current
213-check** catalog (148 EMIR / 65 SFTR) via `cargo bench … --
--quick` (indicative single-shot); the 1k/10k/100k rows are the
prior 2026-05-15 figures (200-check catalog) — kept for trend, not
rewritten. Per-record throughput drops sharply at 1M (EMIR ~73 k vs
~312 k at 100k) as the working set far exceeds cache and allocation
pressure rises — re-run `cargo bench` for current numbers.

SFTR is faster per record because its catalog is smaller (65 vs
148 checks). These numbers are lower than the pre-0.1.0 baseline
because the generators now populate far more fields, so more checks
do real work per record (a more honest figure than the old
sparse-record bench). Throughput is roughly linear up to 100k
(both layers process 100k in well under half a second), then
**degrades super-linearly at 1M** as the working set far exceeds
cache and allocation pressure rises — SFTR 1M ≈ 3 s but EMIR 1M
≈ 14 s (the larger 148-check catalog amplifies the cache effect).
The loop is `rayon`-parallelised over the *checks* dimension —
throughput scales with available cores, but the 1M cliff (and the
end-to-end peak-RSS finding below) is what the deferred perf work
targets.

## End-to-end scale baseline (parse + checks + write; peak RSS)

The check-loop numbers above measure only the in-memory check
dimension. The real scale/memory question — the goal of the
performance/scale work — is the **whole `opendqi scan` pipeline**:
discover → parse XML → run all checks → write `summary.json` /
`issues.csv` / `report.html`, holding the full batch `Vec` in memory.

`scripts/bench-scale.sh` is a deliberate, opt-in **local release**
tool (intentionally **not** in `scripts/preflight.sh` or CI, which
stay debug per build hygiene). It generates synthetic
`auth.030`/`auth.052` XML via the dependency-free
`opendqi-core/examples/gen_synthetic_xml.rs` (deterministic,
streamed), runs the release binary under a peak-RSS + wall-time
wrapper, and prints a table. Generated data goes to a temp dir and is
never committed.

```bash
./scripts/bench-scale.sh            # full: emir|sftr × 100k, 1M
./scripts/bench-scale.sh --smoke    # fast plumbing check (N=200)
```

**Peak-RSS portability:** the script targets macOS `/usr/bin/time -l`
("maximum resident set size" in **bytes**). On Linux substitute
`/usr/bin/time -v` ("Maximum resident set size" in **KB**) — see the
script header. The numbers below were captured on the same
Apple-Silicon box as the check-loop table; **indicative, not a
contract**.

Captured 2026-05-17, same Apple-Silicon box, release `lto=thin`,
single synthetic input file per run:

| Pipeline (parse + checks + write) | Records | Wall time | Peak RSS |
|---|---|---|---|
| `opendqi emir scan` | 100 000 | ~5.2 s | ~0.8 GiB |
| `opendqi emir scan` | 1 000 000 | ~80 s | **~3.9 GiB** |
| `opendqi sftr scan` | 100 000 | ~1.8 s | ~0.33 GiB |
| `opendqi sftr scan` | 1 000 000 | ~26 s | ~2.1 GiB |

Reproducible (the generator is index-driven, no RNG) — re-run
`./scripts/bench-scale.sh` on your hardware. **This is the key
finding that motivates the chantier:** unlike the check loop (fast,
linear, cache-bound), the full scan's **peak RSS grows ~linearly
with record count** — a 1M-record EMIR batch holds ~3.9 GiB resident
(the entire `Vec<EmirRecord>` + every `DqIssue` + the in-memory
report build, none of it streamed). Wall time is dominated by XML
parse + report materialisation, not the (already-parallel) check
loop. Peak RSS, not throughput, is the binding constraint for
"millions of records / memory-bounded processing"; this baseline is
the primary input for the deferred memory-bounded / streaming and
incremental-scan increments.

## Phase attribution (M0.15)

M0.13 measured only the **total** peak. M0.14 then *guessed* the
contributor (per-record `raw_fields`) and was reverted when a probe
proved it ≈ 0 bytes on this workload. M0.15 therefore **measures
which phase owns the peak** before any optimization, via opt-in
phase-boundary instrumentation (`OPENDQI_MEM_TRACE`, surfaced by
`./scripts/bench-scale.sh --mem-trace`). The trace prints **current**
RSS sampled at six `run_scan` boundaries; `/usr/bin/time -l` still
reports the run **maximum**. Captured 2026-05-17, same Apple-Silicon
box, release `lto=thin`. **Indicative, not a contract.**

| regime | records | total peak | post_discovery | post_parse | post_checks | post_lifecycle_presub | post_finalize | post_report |
|---|---|---|---|---|---|---|---|---|
| emir | 100 000 | 808 MiB | 5 | 225 | 784 | 780 | 777 | 543 |
| emir | 1 000 000 | **3423 MiB** | 5 | 149 | 715 | 715 | **1305** | 853 |
| sftr | 100 000 | 346 MiB | 6 | 205 | 338 | 338 | 338 | 343 |
| sftr | 1 000 000 | **2189 MiB** | 5 | 834 | **1992** | 1992 | 1992 | 874 |

(boundary columns are *current* RSS in MiB at that point.)

**Key finding — the dominant phase differs by regime:**

- **SFTR 1M**: total peak (2189) ≈ `post_checks` current (1992). The
  peak is the **parse + checks steady state** — the resident
  `Vec<SftrRecord>` (post_parse already 834) plus the accumulating
  `Vec<DqIssue>` (checks add ~1.2 GiB). It then **drops to 874 at
  post_report** (the finalize/report phase is cheap for SFTR — the
  big Vecs are still resident but no large transient is added).
- **EMIR 1M**: total peak (**3423**) is **far above every boundary
  sample** (highest is `post_finalize` 1305). The persistent resident
  climbs parse(149) → checks(715) → finalize(**1305**) then **falls
  to 853 at post_report** — so the 3423 maximum is a **large
  transient (~2 GiB above the post_finalize resident) that occurs
  *between* `post_finalize` and `post_report` and is released before
  the post_report sample**: i.e. inside `finalize_issues` (global
  severity-override + sort over the full `Vec<DqIssue>`) and/or the
  three `write_*` calls. The 6-point trace **cannot split finalize
  vs. report** — that span is the next increment's target and needs
  finer probes (it does *not* implicate the records `Vec`, which is
  flat from post_checks on and freed regardless at report).

**Honest limitation:** boundary samples are *points*; a transient
that spikes and is freed *between* two samples (the EMIR 1M case) is
invisible to the 6-point trace except as a max/sample gap. This is
expected and is exactly the signal that localizes the next probe —
not a defect in the method.

**Direction for the next increment (data-driven, not guessed):**

1. EMIR: add finer instrumentation *inside* the finalize→report span
   (around `finalize_issues` and each of `write_summary_json` /
   `write_issues_csv` / `write_report_html`) to pin the ~2 GiB
   transient, then bound it (e.g. stream `issues.csv` row-by-row
   without the intermediate sorted `Vec`, avoid a full-issue clone in
   finalize/sort).
2. SFTR: the binding constraint is the **steady-state working set**
   (`Vec<*Record>` + `Vec<DqIssue>` co-resident through checks). The
   `records` Vec is provably dead after the check passes (the
   `write_*` writers do not take `&records`) — freeing it before
   report-write is a candidate, but the trace shows report is already
   the *low* point, so the real lever is not holding records **and**
   all issues **and** the report build simultaneously; that needs the
   chunked-checks / spill design (whole-batch checks —
   `DUPLICATE_UTI`, lifecycle — are the documented hard constraint).

No optimization was applied in M0.15 (measurement only, by design —
the M0.14 lesson). The instrumentation is opt-in and output-invariant
(unset env ⇒ byte-identical scan output; golden / XSD-conformance
unchanged).

## True-peak attribution (M0.16)

M0.15 localized the EMIR peak only to "somewhere in finalize→report"
— the 6-point boundary trace structurally cannot see a transient
freed *between* samples. M0.16 adds (a) **four finer markers**
(`post_summary`, `post_write_summary_json`, `post_write_issues_csv`,
`post_write_report_html`) and (b) a **background peak sampler** that
polls RSS (default 200 ms; `OPENDQI_MEM_TRACE_MS`) and reports the
run **maximum** plus the phase marker live at that instant — catching
a transient the boundary trace misses. Captured 2026-05-17, same
Apple-Silicon box, release `lto=thin`, 100 ms sampler.
**Indicative, not a contract.**

Finer boundary trace (current RSS, MiB):

| regime | recs | parse | checks | lifecycle | **finalize** | summary | wr_json | wr_csv | wr_html | report |
|---|---|---|---|---|---|---|---|---|---|---|
| emir | 100k | 238 | 882 | 882 | 882 | 877 | 878 | 771 | 781 | 781 |
| emir | 1M | 179 | 1471 | 1471 | **3819** | 3819 | 3819 | 1585 | 2492 | 2492 |
| sftr | 100k | 205 | 335 | 335 | 335 | 335 | 335 | 337 | 338 | 338 |
| sftr | 1M | 1135 | 2241 | 2241 | 2241 | 2241 | 2242 | 1916 | 1924 | 1924 |

Background sampler — true peak vs. independent `/usr/bin/time -l`:

| regime | recs | sampler peak | OS max | phase live at peak |
|---|---|---|---|---|
| emir | 100 000 | 897 MiB | 914 | post_parse (during checks) |
| emir | 1 000 000 | **3901 MiB** | 3998 | post_write_summary_json (≈ entering write_issues_csv) |
| sftr | 100 000 | 338 MiB | 338 | post_report |
| sftr | 1 000 000 | 2432 MiB | 2432 | post_parse (during checks) |

**Resolved finding — the EMIR 1M culprit is `finalize_issues`:**

- Resident **jumps 1471 → 3819 MiB across `finalize_issues`** (a
  **persistent ~2.35 GiB**, not a brief spike — it stays 3819 through
  `post_summary`/`post_write_summary_json`), then **`write_issues_csv`
  frees ~2.2 GiB** (3819 → 1585). The sampler's true peak (3901)
  ≈ the OS max (3998) and is attributed to the finalize→
  `write_issues_csv` span. So `finalize_issues` (the global
  severity-override + sort over the full `Vec<DqIssue>`) **roughly
  doubles the working set at scale**; the writes are secondary
  (`write_issues_csv` is actually where memory is *released*).
- **Scale-dependent**: at 100k the finalize jump is **absent**
  (882 → 882). The blow-up appears only at 1M ⇒ an O(n)/clone-shaped
  cost proportional to issue count (millions of `DqIssue` at 1M) —
  the precise hypothesis the next increment tests against
  `finalize_issues`' implementation.
- **SFTR**: confirms M0.15 — true peak (2432) occurs while
  `phase=post_parse` is live, i.e. *during the check phase*: the
  `Vec<SftrRecord>` (post_parse 1135) plus the accumulating
  `Vec<DqIssue>`. `finalize_issues` is flat for SFTR (2241 → 2241):
  the EMIR blow-up is **not** intrinsic to finalize but scales with
  the EMIR issue volume specifically.

**Method validated / honest limits:** the sampler max (3901 MiB)
independently agrees with `/usr/bin/time -l` (3998 MiB) within ~2 %,
and it correctly split the span the M0.15 trace could not. Boundary
samples are still points (the `post_finalize` 3819 is what finalize
leaves *resident* — a persistent climb, more actionable than a
spike); the 100 ms sampler and its own macOS `ps` children perturb
the measured process slightly (the ~2 % sampler-vs-OS gap bounds it).

**Next increment (data-driven):** bound `finalize_issues` for EMIR —
inspect `crates/opendqi-core/src/dq/mod.rs::finalize_issues` for the
~2.35 GiB scale allocation (a full-issue clone / O(n) auxiliary in
the severity-override or sort over millions of `DqIssue`) and remove
it (e.g. sort/override in place; avoid an allocating sort key). SFTR
remains the separately-scoped chunk/spill (records+issues
co-residence; whole-batch checks are the hard constraint).

No optimization was applied in M0.16 (measurement only, by design —
the M0.14/M0.15 discipline). Opt-in and output-invariant: env unset
⇒ no sampler thread, markers are no-ops ⇒ byte-identical scan output
(golden / XSD-conformance unchanged); not in preflight/CI.

## Conventions

- All new checks must be `Send + Sync` (compiler-enforced at the
  trait definition).
- Checks must not capture mutable state outside their input slice
  and `CheckContext` — they're called in parallel.
- The post-pass `finalize_issues` (severity overrides + sort) is the
  source of truth for issue ordering. Tests must not assume any
  pre-sort order.
- New `run_all*` variants should follow the same `par_iter ->
  flat_map_iter -> finalize_issues` shape.

## What's not parallelised (v1)

- **File ingestion**: still sequential. XML/CSV reading is mostly
  I/O bound; a typical scan has < 10 input files.
- **Records dimension**: each check still iterates records
  sequentially. The vast majority of checks are O(n) with very
  small constants; record-level parallelism would add scheduling
  overhead for negligible gain.
- **`sort_issues`**: stays single-threaded. The cost is dominated
  by collection growth, not the sort itself.

The end-to-end baseline above shows the binding constraint is **peak
memory**, not the check loop: the whole batch is materialised as one
`Vec<*Record>` and the full issue list + report are built in memory,
so RSS grows ~linearly (~3.9 GiB at 1M EMIR records). The deferred
performance increments — memory-bounded / streaming scan and an
incremental (changed-inputs-only) scan mode — are scoped against
**this measured baseline**; re-run `scripts/bench-scale.sh` after
each to quantify the improvement. (This milestone only *measures* —
no optimization yet, by design.)
