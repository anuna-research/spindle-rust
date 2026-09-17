# How to Tune Reasoning Performance

Measure complete preparation and reasoning on representative theories. Grounding,
conflict resolution, and aggregate stages can dominate different workloads.

## Start with a baseline

```sh
cargo build --release -p spindle-cli
./target/release/spindle stats theory.spl
time ./target/release/spindle reason theory.spl > /dev/null
make bench
make bench-scaling
```

When comparing revisions, use the same toolchain, machine, and fixture. A short
local benchmark is not a portable performance guarantee.

## Control grounding

Independent variables multiply candidate combinations. For example, with 100
nodes this rule can produce 10,000 pairs:

```spl
(normally pairs (and (node ?x) (node ?y)) (pair ?x ?y))
```

When selective relations and guards express the intended domain, use them.
Bind expression inputs before evaluating them. Configure grounding limits via
`PrepareOptions` in Rust. Splitting a rule or adding priorities can change
its defeasible semantics. After a rewrite, check conclusions as well as runtime.

## Aggregate workloads

```sh
make bench-aggregation
cargo bench -p spindle-core --bench aggregation -- --test
cargo bench -p spindle-core --bench aggregation -- --save-baseline before
# After changing the implementation:
cargo bench -p spindle-core --bench aggregation -- --baseline before
```

The aggregate suite measures preparation and full reasoning separately. It
covers reducer choice, row count, grouping, unrelated facts, and chained stages.
Fixture creation and expected-output checks happen outside timed loops;
timed loops include allocation and destruction. See the
[initial baseline and methodology](../reference/aggregation-benchmarks.md).

Repeated stages can require grounding and reasoning earlier prefixes again.
Grounding can instantiate cyclic predicates over source and computed constants.
These combinations can grow exponentially. `max_instances` bounds aggregate grounding work across
repeated passes; exhaustion is an error, not a partial answer.

## Memory and integration

The engine uses interned names, compact indexes, bitsets, and small-vector rule
storage. These reduce overhead but do not eliminate the cost of a large grounded
theory. `reason()` returns a collected result; there is no public `reason_iter()`
streaming API. Unless your application needs cloned theories or full result histories, avoid retaining them.

For heap profiling, use `make bench-memory`. Criterion output lives under
`target/criterion/`. In browser applications, run expensive synchronous WASM
reasoning in a worker to keep the UI responsive.
