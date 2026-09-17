# How to benchmark aggregation

Run these commands from the repository root on an otherwise idle machine.

## Check fixtures

Before collecting timings, check every fixture's expected results:

```sh
cargo bench -p spindle-core --bench aggregation -- --test
```

The command checks all fixtures without collecting timings.

## Measure the current revision

Run the dedicated Criterion suite:

```sh
make bench-aggregation
```

Criterion writes reports under `target/criterion/`.
The [benchmark reference](../reference/aggregation-benchmarks.md) describes coverage, timing boundaries, defaults, and the initial local baseline.

## Compare a change

Save a baseline before changing the code:

```sh
cargo bench -p spindle-core --bench aggregation -- --save-baseline before
```

After changing the code, compare against that baseline:

```sh
cargo bench -p spindle-core --bench aggregation -- --baseline before
```

For more precise comparisons, increase the sampling budget.
This command measures one family with 30 samples and a 5-second measurement target:

```sh
cargo bench -p spindle-core --bench aggregation -- aggregation/groups --sample-size 30 --measurement-time 5
```
