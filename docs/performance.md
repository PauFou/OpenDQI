# Performance — parallelism + benchmarks

OpenDQI's check loop fans out over the dimension `checks`, not
`records`. Records are small structs (40-50 typed fields); the
catalog is 225+ EMIR / 98+ SFTR checks running mostly O(n) over
records. Parallelizing the **checks** dimension is the high-leverage
choice: scheduling overhead amortises across `n_records` iterations
of each check.

## Implementation

All 17 `run_all*` runners in `crates/opendqi-core/src/dq/mod.rs` use
`rayon::par_iter().flat_map_iter(...)` :

```rust
let mut issues: Vec<DqIssue> = checks
    .par_iter()
    .flat_map_iter(|c| c.run(records, ctx))
    .collect();
sort_issues(&mut issues);
```

- `par_iter` distributes the check evaluations across the rayon
  global thread pool (defaults to one worker per logical core).
- `flat_map_iter` accepts each check's `Vec<DqIssue>` as a plain
  iterator, avoiding the cost of turning each tiny sub-Vec into a
  `ParallelIterator`.
- The final `sort_issues` is sequential and runs in
  O(n log n) over the merged issue list — typically a few thousand
  items max, dwarfed by the check loop.

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

Two suites are exercised at 1k and 10k synthetic records each:

- `run_all_emir/1000` and `/10000` — `default_checks()` (the full
  EMIR catalog).
- `run_all_sftr/1000` and `/10000` — `default_sftr_checks()`.

Synthetic records are deterministic; each iteration produces the
same input. Criterion handles warm-up, calibration, and outlier
detection.

## Reference numbers

These are local-machine numbers (Apple Silicon, Rust 1.75, release
build) measured after the Phase 8.6 commit. They scale roughly
linearly with `n_records` once warm.

| Workload | Records | Wall time | Throughput |
|---|---|---|---|
| `run_all_emir` | 1 000 | ~2.4 ms | ~415 k records/s |
| `run_all_emir` | 10 000 | ~24.6 ms | ~407 k records/s |
| `run_all_sftr` | 1 000 | ~0.96 ms | ~1.04 M records/s |
| `run_all_sftr` | 10 000 | ~7.9 ms | ~1.27 M records/s |

SFTR is faster per record because its catalog is smaller (~98 vs
~225 checks). Both layers stay well under 100 ms on 10 k records,
so even a million-record batch runs in seconds.

## Conventions

- All new checks must be `Send + Sync` (compiler-enforced at the
  trait definition).
- Checks must not capture mutable state outside their input slice
  and `CheckContext` — they're called in parallel.
- The post-sort `sort_issues` is the source of truth for issue
  ordering. Tests must not assume any pre-sort order.
- New `run_all*` variants should follow the same `par_iter ->
  flat_map_iter -> sort_issues` shape.

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
