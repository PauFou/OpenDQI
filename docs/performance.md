# Performance — parallelism + benchmarks

OpenDQI's check loop fans out over the dimension `checks`, not
`records`. Records are small structs (40-50 typed fields); the
catalog is 199 checks (135 EMIR + 64 SFTR) running mostly O(n) over
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

Two suites are exercised at 1k, 10k, and 100k synthetic records each:

- `run_all_emir/{1000,10000,100000}` — `default_checks()` (the full
  135-check EMIR single-batch catalog).
- `run_all_sftr/{1000,10000,100000}` — `default_sftr_checks()` (the
  64-check SFTR catalog).

The synthetic generators are **deterministic** (index-driven, no
RNG) and populate ~30 typed fields per record so the vast majority
of checks evaluate non-trivially rather than short-circuiting on
absent data. Criterion handles warm-up, calibration, and outlier
detection.

## Reference numbers

Local-machine numbers (Apple Silicon, Rust stable, release build,
`lto=thin`), measured 2026-05-15 against the full 199-check
catalog with the enriched deterministic generators. They scale
roughly linearly with `n_records` once warm. **Indicative, not a
benchmark contract** — re-run `cargo bench` on your hardware.

| Workload | Records | Wall time | Throughput |
|---|---|---|---|
| `run_all_emir` | 1 000 | ~2.4 ms | ~411 k records/s |
| `run_all_emir` | 10 000 | ~23.4 ms | ~428 k records/s |
| `run_all_emir` | 100 000 | ~320 ms | ~312 k records/s |
| `run_all_sftr` | 1 000 | ~1.5 ms | ~684 k records/s |
| `run_all_sftr` | 10 000 | ~12.8 ms | ~779 k records/s |
| `run_all_sftr` | 100 000 | ~164 ms | ~609 k records/s |

SFTR is faster per record because its catalog is smaller (64 vs
135 checks). These numbers are lower than the pre-0.1.0 baseline
because the generators now populate far more fields, so more checks
do real work per record (a more honest figure than the old
sparse-record bench). Throughput dips slightly at 100k as the
working set exceeds L2/L3 cache; it remains comfortably linear.
Both layers process 100 k records in well under half a second, so a
million-record batch runs in a few seconds. The loop is
`rayon`-parallelised over the *checks* dimension — throughput scales
with available cores.

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

Future milestones may revisit these if needed (e.g. very large
single files, batches > 1 M records).
