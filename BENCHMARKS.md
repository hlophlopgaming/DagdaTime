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
cargo bench --offline -p draugr-time-core --bench reports -- --baseline original --save-baseline flat
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
