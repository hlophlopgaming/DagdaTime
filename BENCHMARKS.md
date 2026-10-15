# M7 report calculation benchmarks

2026-10-15. Linux x86_64, AMD Ryzen 5 3600, rustc 1.99.0.
Criterion 0.5.1, release defaults; no custom compiler flags. Nine independent
cases, 30 samples, 1 s warm-up, 2 s requested measurement time (Criterion
extends it where necessary). Benchmarks and tests run sequentially.

Scope: shared `Report::calculate` used by desktop reports and daemon history,
and the same calculation used by storage-backed CLI reports. No schema,
platform or output-format changes. Target: at least 1.2x per case, retaining
full output equivalence; report any case that misses the target honestly.

Inputs are constructed outside timing: 16, 5,000 and 25,000 window events,
32 hierarchical tags, interleaved tag ranges and web observations. Sequential
and overlapping-window cases reuse document names; unique cases do not.
`overlapping` means windows, not densely overlapping tag ranges. Tags, gaps,
unknown context, ties, clipping and arbitrary input order are separately checked
against a frozen original sweep on 192 non-benchmark cases and existing tests.
Every timed iteration computes a fresh report; no cross-iteration cache.

## Commands

```sh
cargo bench --offline -p draugr-time-core --bench reports -- --save-baseline original
cargo bench --offline -p draugr-time-core --bench reports -- --save-baseline flat
cargo bench --offline -p draugr-time-core --bench reports -- --save-baseline buckets
```

## Baseline

Criterion arithmetic mean; times include report construction and destruction,
exclude fixture construction. Production implementation unchanged for baseline.

| Shape | Window events | Original mean (ms) |
| --- | ---: | ---: |
| overlapping | 16 | 0.021213 |
| overlapping | 25000 | 36.841589 |
| overlapping | 5000 | 6.437314 |
| sequential | 16 | 0.017621 |
| sequential | 25000 | 26.590055 |
| sequential | 5000 | 4.909979 |
| unique | 16 | 0.017622 |
| unique | 25000 | 29.864610 |
| unique | 5000 | 5.540439 |

## Pass 1: flat boundary sweep

Replace the timestamp tree, per-timestamp vectors and copied keys with one
sorted vector grouped by timestamp. Apply all equal-time changes before emitting
the following interval; retain `(id, input_index)` winner precedence. Missing
outer sentinels cannot hide active time: every contributing clipped interval
already contributes both endpoints. All 61 core tests pass.

| Shape | Window events | Flat mean (ms) | Speedup vs original |
| --- | ---: | ---: | ---: |
| overlapping | 16 | 0.018510 | 1.146x |
| overlapping | 25000 | 25.203247 | 1.462x |
| overlapping | 5000 | 4.728251 | 1.361x |
| sequential | 16 | 0.016010 | 1.101x |
| sequential | 25000 | 20.384790 | 1.304x |
| sequential | 5000 | 3.708284 | 1.324x |
| unique | 16 | 0.015808 | 1.115x |
| unique | 25000 | 23.041468 | 1.296x |
| unique | 5000 | 4.207552 | 1.317x |

Tiny cases miss the 1.2x target; retain the improvement and measure bucket dispatch next.

## Pass 2: dedicated bucket sets

Replace the dynamic bucket map with the four existing report context sets.
Unknown bucket endpoints remain in the boundary list; only unused membership
updates disappear. Keep overlap ordering and all validation unchanged.
All 61 core tests pass; two independent read-only reviewers found no regression.

| Shape | Window events | Bucket mean (ms) | Speedup vs original | Speedup vs pass 1 |
| --- | ---: | ---: | ---: | ---: |
| overlapping | 16 | 0.014069 | 1.508x | 1.316x |
| overlapping | 25000 | 17.950840 | 2.052x | 1.404x |
| overlapping | 5000 | 3.229233 | 1.993x | 1.464x |
| sequential | 16 | 0.012425 | 1.418x | 1.289x |
| sequential | 25000 | 15.227958 | 1.746x | 1.339x |
| sequential | 5000 | 2.609913 | 1.881x | 1.421x |
| unique | 16 | 0.012526 | 1.407x | 1.262x |
| unique | 25000 | 19.346582 | 1.544x | 1.191x |
| unique | 5000 | 3.155522 | 1.756x | 1.333x |

## Pass 3: rejected range-root cache; convergence

Precompute each range's resolved tag root to avoid per-span hash lookups.
The differential test passes, but the extra vector slows several cases and
larger-case improvements are only a few percent. Revert this experiment; the
retained implementation is pass 2. No benchmark input or measurement setting
changed in any pass.

| Shape | Window events | Experiment mean (ms) | Speedup vs pass 2 |
| --- | ---: | ---: | ---: |
| overlapping | 16 | 0.014729 | 0.955x |
| overlapping | 25000 | 17.002190 | 1.056x |
| overlapping | 5000 | 3.313659 | 0.975x |
| sequential | 16 | 0.012867 | 0.966x |
| sequential | 25000 | 15.164498 | 1.004x |
| sequential | 5000 | 2.583311 | 1.010x |
| unique | 16 | 0.012959 | 0.967x |
| unique | 25000 | 18.654631 | 1.037x |
| unique | 5000 | 3.324364 | 0.949x |

Stop after this diminishing-return pass. The production sweep remains about the
same source size as the original; no broad refactor or unrelated milestone work
is justified. Two read-only reviewers rechecked the retained implementation.

## Reproduction and limits

Run `original` at commit `b3aa53c`, `flat` at `6471bd0` and `buckets` at
`1d95998`. Criterion raw estimates and 95% confidence intervals are in
`target/criterion/report_calculate/<shape>/<size>/<baseline>/estimates.json`
(local build artifacts). Pass 3 used the same command with `--save-baseline roots`.
A future rerun must preserve the benchmark file from the baseline commit.

These are isolated in-memory calculation measurements on one Linux machine,
not end-to-end UI/SQLite or Windows/macOS latency claims. Synthetic inputs do
not characterize arbitrarily deep tag trees, dense tag overlap or all database
sizes. Existing storage/chunking, interval, DST and CLI integration tests cover
those output paths; real native performance remains to be measured on each OS.
No platform code changed; manual checks can navigate Day overview, Week and
Reports on native Windows/macOS and confirm displayed totals and responsiveness.

## Final validation

```text
cargo fmt --check
  exit 0 (no formatting differences)
cargo clippy --offline --all-targets -- -D warnings
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.05s
cargo test --offline
  Finished `test` profile [unoptimized + debuginfo] target(s) in 26.77s
  186 passed; 0 failed; 7 ignored across the workspace
```

Ignored tests require native desktop/session fixtures or isolated process-language
execution. No ignored test was enabled as part of this report-only change.
The retained production source matches pass 2 exactly after reverting pass 3.
Benchmark file diff against `b3aa53c` is empty.
