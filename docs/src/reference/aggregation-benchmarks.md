# Aggregation benchmark measurements

The suite measures `prepare` (including aggregate snapshots and grounding) and
`reason` (preparation plus final reasoning) separately. Parsing, fixture creation,
and expected-result assertions run outside the timed loops. These are pipeline
measurements, not isolated reducer timings; allocation and result destruction are
included. Each fixture checks the complete set of positive aggregate outputs
through both entry points.

Coverage includes these fixture families:

| Dimension | Values |
|---|---|
| Rows, for each of the four named reducers | 0, 100, 500, 1,000 |
| Groups sharing 500 rows | 1, 10, 100 |
| Unrelated facts alongside 100 matching rows | 0–1,000 |
| Aggregate stages with 100 rows and 10 groups | 1, 2, 4 |

Unique row IDs
preserve repeated equal contributions. Chained stages consume derived results.

## Sampling and output

Defaults use 10 samples, 0.5 seconds of warmup, and a 1-second measurement target per case.
Expensive cases take longer when their iterations exceed that target.
Criterion reports and local baselines reside under `target/criterion/`.

## Initial local baseline

The initial run used an Apple M2, rustc 1.95.0, and the optimized bench profile on 2026-09-13.
It used the defaults above with `--save-baseline initial`. All 50 timed cases passed
their output checks. Selected Criterion mean estimates (milliseconds):

| Fixture | Prepare | Full reasoning |
|---|---:|---:|
| Sum, 100 rows, 1 group | 4.37 | 7.35 |
| Sum, 1,000 rows, 1 group | 207.51 | 360.25 |
| Sum, 500 rows, 1 group | 59.40 | 102.28 |
| Sum, 500 rows, 100 groups | 164.36 | 236.20 |
| Sum, 100 rows, 1 group, 1,000 unrelated facts | 192.71 | 343.89 |
| Sum, 100 rows, 10 groups, 1 stage | 6.17 | 9.88 |
| Sum, 100 rows, 10 groups, 4 stages | 20.33 | 25.23 |

These short local measurements are a starting point, not portable performance
thresholds. Row and unrelated-fact scaling warrant profiling. The measurements
include grounding and reasoning costs, so they do not establish that fold scans
alone cause the growth. Increasing group count also adds group facts and outputs;
increasing stage count adds rules and intermediate outputs.

[Benchmarking aggregation](../guides/benchmark-aggregation.md) describes fixture checks, baseline comparisons, and longer measurements.
