# Local smoke measurements

Measured on 2026-09-16, Apple M4, 16 GiB RAM, unoptimized Rust build with debug
assertions enabled and debug information disabled. These measurements include
fresh parameter/key generation on each prove/verify call. They are not a
production throughput claim and do not measure the worst inference graph.

Reproduce with:

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo run -p spindle-zk --example profile -- 0 8 63
```

The policy has a public definite query and a varying private candidate schema.
All candidates are selected for proving. The final case reaches the current
literal-universe limit: candidate literals, their complements, and the public
query with its complement. It exercises commitment scaling, not hard inference.

| Candidate facts | Boolean operations | Compile ms | Prove ms | Verify ms | Transcript bytes |
|---|---|---|---|---|---|
| 0 | 2 | 2 | 1349 | 870 | 2784 |
| 8 | 18 | 0 | 5123 | 3800 | 2912 |
| 63 | 128 | 2 | 39283 | 33677 | 3104 |

Millisecond rounding explains the zero compilation measurement. JSON envelope
size is deliberately not treated as fixed because decimal encodings of random
bytes vary in length. The example also emits package sizes for inspection.

The measurements support small bounded experiments, not unattended hostile-policy
verification. Literal/rule/gate limits are structural rejection limits, not a
latency guarantee. The later maximum gate-budget measurement is recorded below; limits
must not be advertised as performance-certified. Key caching and tighter layout
sizing may improve performance, but neither is implemented or assumed here.

## Dense compilation boundary

The separately runnable resource test uses the full literal universe and dense
cyclic two-premise rules, with every positive atom a possible private fact:

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 cargo test -p spindle-zk --test resources -- --include-ignored --nocapture
```

| Rules | Literals including complements | Compiled Boolean operations | Compile ms |
|---|---|---|---|
| 64 | 128 | 98114 | 121 |
| 128 | 128 | 163458 | 167 |
| 256 | 128 | 294146 | 303 |

All completed without truncating the inference relation. The same test target
confirms that an additional rule beyond the rule limit, or enough candidate
facts to exceed the literal limit, returns `ResourceLimit`. These measurements
exercise maximum rule/literal counts in one family, not the maximum gate budget,
and do not measure proving those dense circuits.

## End-to-end dense inference

The `dense_profile` example uses the same cyclic two-premise rule family, with
four rules per positive atom. Unlike the commitment-only profile, it omits the
query `p0` from the private facts and proves its defeasible conclusion through
inference. Every reported sample includes successful verification and fresh key
generation; failed proofs do not produce a result row.

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo run -p spindle-zk --example dense_profile -- 8
CARGO_INCREMENTAL=0 cargo run -p spindle-zk --release --example dense_profile -- 8 16 32 64
```

An unoptimized smoke run on the machine above measured 3,254 Boolean operations
for 8 positive atoms and 32 rules: compilation 5 ms, proving 22,139 ms,
verification 15,572 ms, transcript 3,040 bytes. Other test/build processes ran
concurrently, so this is a functional scaling observation, not an isolated
benchmark or a comparison with the earlier measurements.

The optimized run (`--release`, workspace LTO enabled) completed these samples:

| Positive atoms | Rules | Boolean operations | Compile ms | Prove ms | Verify ms | Transcript bytes |
|---|---|---|---|---|---|---|
| 8 | 32 | 3254 | 1 | 2123 | 1501 | 3040 |
| 16 | 64 | 15194 | 1 | 8491 | 6266 | 3168 |
| 32 | 128 | 73346 | 6 | 35425 | 28336 | 3296 |

The 64-atom/256-rule sample compiled to 294,146 operations, but was stopped
with SIGTERM during proving after severe host memory pressure. At observation,
the process was in uninterruptible wait, system compressed-memory occupancy
was approximately 11 GiB, and filesystem free space had fallen to 240 MiB.
These are whole-host observations, not measured peak memory attributable solely
to the prover. The host was not isolated from other applications. No proof or
verification timing is reported for that sample, and it MUST NOT be counted as
a successful boundary proof. The benchmark process exited with status 143.

At that checkpoint, even the maximum rule/literal workload in this family had
not completed end-to-end on this host. The one-million-operation rejection ceiling
is **not yet a benchmark-validated operational limit**. Resolving that gap needs
further layout/memory work or controlled measurements with sufficient resources;
the successful smaller samples do not discharge it.

Subsequent claim-specific dependency pruning removes disconnected inference
but retains every committed input and global aggregate validity check. The
example now reports both full-policy `operations` and retained
`claim_operations`. For the dense 64-atom sample, those counts are 294,146 and
292,134 respectively: this strongly connected workload does not benefit much.
That rerun was deliberately stopped before completion to avoid repeating host
memory exhaustion. The timings above predate pruning; they are historical
baseline measurements, not measurements of the updated circuit.

## Maximum-budget layout check

The ignored unit test `maximum_budget_layout_is_synthesizable` creates a
synthetic dependency chain using NOT, AND and OR gates, with 63 declared private
candidates plus a public query. It exercises exactly 1,000,000 Boolean
operations, including a dependency on the final wire, without pruning the test
graph. This is a backend envelope test, not a source-program benchmark.

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 cargo test -p spindle-zk --lib maximum_budget_layout_is_synthesizable -- --ignored --nocapture
```

Observed on the same machine: `k=20`, predicted transcript 3,488 bytes,
public-layout measurement 2,177 ms. The measurement completed without a
layout-sizing panic. It neither creates a cryptographic proof nor establishes
the peak memory or runtime of maximum-budget proving.

After sharing advice columns between inference and Poseidon regions, the same
layout check completed at `k=20`, predicted transcript 3,072 bytes, measurement
1,908 ms. The fixture's final wire is now an OR with true so it also admits a
known valid witness for the separate real-proof boundary test. These results
still measure layout only.

The separately runnable `maximum_budget_proof_roundtrip` test generates and
verifies an actual proof of that synthetic envelope, independently regenerating
the verification key. It reuses transparent parameters for verification, unlike
the end-to-end CLI/API profiles which regenerate those too:

```sh
CARGO_INCREMENTAL=0 RAYON_NUM_THREADS=2 cargo test -p spindle-zk --release --lib maximum_budget_proof_roundtrip -- --ignored --nocapture
```

The first executed run is recorded below; it did not complete successfully.
The test's existence is not passing performance evidence. It can require substantial memory and runtime;
do not include ignored resource tests in an unattended default test run.

## Completed dense boundary baseline

A subsequent two-worker run completed the 64-atom/256-rule policy, using public
claim pruning but the older separate inference/Poseidon advice columns:

```sh
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 cargo build -p spindle-zk --release --example dense_profile
RAYON_NUM_THREADS=2 /usr/bin/time -l target/release/examples/dense_profile 64
```

Observed: full graph 294,146 operations, selected claim 292,134 operations,
compilation 28 ms, proving 373,250 ms, verification 262,960 ms, transcript 3,424
bytes, successful exit status 0. Fresh parameter and key generation are included
in both API operations. Whole-process wall time was 636.65 seconds.

macOS `time -l` reported maximum resident set size 3,972,579,328 bytes and peak
memory footprint 11,767,227,296 bytes. These are different OS metrics and must
not be conflated. A stack sample during proving located verification-key
commitment work; RSS snapshots alone understated the physical-footprint metric.
The host was not isolated, and small test/compile jobs ran concurrently, so this
is not a controlled throughput comparison with other rows.

This establishes successful end-to-end execution of this dense boundary policy,
not of every policy at the one-million-operation ceiling. The shared-column
optimization was implemented while the existing binary ran; this result is its
pre-optimization baseline, not evidence of a memory reduction from that change.

The subsequent two-lane layout packs two operations into each inference row
and reuses the first lane for Poseidon state. The million-operation layout test
now measures `k=19`, predicted transcript 3,456 bytes, and 1,934 ms synthesis
measurement. This halves the padded row domain relative to the earlier `k=20`
fixture without lowering the operation ceiling. It is a layout result, not yet
an end-to-end memory or proving measurement. Ground/aggregate actual proofs,
corrupted witnesses, and explicit odd/even final-row cases passed. Independent
source review found no blocker in lane indexing, same-row copies, selectors,
public-cell placement, or the proof-length correction.

## Maximum-budget proof attempt

The packed optimized test was explicitly executed with two worker threads and
`/usr/bin/time -l`. Key generation completed in 272,932 ms. The process then
terminated abnormally without reporting a proof result or test assertion failure.
The timing wrapper exited 1 after 316.90 seconds, reporting maximum resident set
size 3,488,645,120 bytes and peak memory footprint 11,776,746,688 bytes. Its signal
diagnostic was `Invalid argument`. A subsequent unified system-log check resolved
the cause: at 2026-09-16 16:35:54.928 the kernel reported
`memorystatus: killing largest compressed process spindle_zk-90fadfe59ecf3c57 [16287] 9741 MB`.
This was a memory-pressure kill, not a passing boundary test.

The next layout uses four independently selected inference lanes. The same
million-operation fixture measures `k=18`, predicted transcript 4,288 bytes,
and 1,919 ms layout measurement. All active module tests and the explicitly run
Lean oracle passed.

## Successful four-lane maximum-budget proof

The optimized test subsequently completed with `RAYON_NUM_THREADS=2`:

```text
maximum_proof keygen_ms=133747 prove_ms=35039 verify_with_keygen_ms=10587 transcript_bytes=4288
test result: ok. 1 passed; 0 failed
180.00 real seconds
7129333760 maximum resident set size (bytes)
8926386872 peak memory footprint (bytes)
```

The test executes all 1,000,000 Boolean operations with 63 candidate inputs,
creates a real proof, checks its length and typed encoding, independently
reconstructs a verification key, and verifies the proof. It reuses transparent
parameters for verification, unlike the public API's fresh verification setup.
This is a synthetic backend envelope, not a source-policy benchmark or a bound
on all workloads. Host activity was not isolated; a small regression-test build
overlapped the tail of the run. These are observed resource costs, not an SLA.
All builds and caches remained on the system disk. The operation ceiling was
not lowered. The prior two-lane memory-pressure failure remains part of the
evidence; layout alone was not treated as proof of memory improvement.
