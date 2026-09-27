# Process-mining review

Reviewed 2026-09-27 on `review/process-mining`, at
`0c9cc446749f8ed523b0327bad7af84f54a8156f`.

**Useful as an exploratory activity-pattern library; not currently reliable as
a process-model-to-defeasible-rule pipeline.** The footprint and trace-support
primitives are worth retaining. The Alpha implementation is incomplete, and
the generated rules do not preserve process control flow. Automatic use of
these rules in a theory can produce conclusions contrary to the observed process.

Scope: `crates/spindle-core/src/mining.rs`, its tests, the mining guide, public
API integration, and execution of the generated rules through `Theory::reason`.
The rule-analysis convenience wrappers delegate to `analysis`; this is not a
separate audit of that module or of the reasoning engine.

## Findings

### P1: rule conversion loses choice and synchronization

Location: `crates/spindle-core/src/mining.rs:836–864`, especially 856–860;
pipeline integration at 967–973.

Despite its name, `petri_net_to_rules` accepts no Petri net. It emits one
independent, positive unary implication for each causal footprint pair.

- With traces `start, approve, end` and `start, reject, end`, mining with
  support 1/confidence 0 produces `start => approve` and `start => reject`.
  Adding the rules and fact `start` to a theory derives **both** outcomes.
  The separate `Conflict` records have no effect on reasoning.
- With traces `start, a, b, end` and `start, b, a, end`, the net represents
  parallel work followed by a join. The rules include `a => end` and
  `b => end`. Fact `a` alone derives **end**, without completing `b`.

Both effects were reproduced through the actual reasoning engine. Support and
confidence only filter inclusion; they are not weights used during inference.
The rules also omit case identifiers, event positions and event bindings.

Decide whether the intended product is descriptive association candidates or
executable process semantics. For candidates, rename/document the conversion
and require explicit adoption by a caller. For process semantics, define a
state/occurrence representation and preserve joins and guarded alternatives;
merely adding superiority relations cannot make distinct positive activity
names logically exclusive. Raising the threshold does not repair the mapping.

### P1: greedy expansion misses valid maximal Alpha pairs

Location: `crates/spindle-core/src/mining.rs:477–500`.

Each causal seed is expanded once, filling A before B. This does not enumerate
all maximal valid pairs. The deterministic counterexample is this log:

```text
s a x e
s a y e
s b x e
s b y e
s c x e
s d y e
```

The valid maximal pair `({a,b}, {x,y})` is missing. Every seed ending in `x`
absorbs `c`, preventing expansion to `y`; every seed ending in `y` absorbs `d`,
preventing expansion to `x`. Actual internal places are
`({s},{a,b,c,d})`, `({a,b,c},{x})`, `({a,b,d},{y})`, and `({x,y},{e})`.
The fifth internal place `({a,b},{x,y})` is never created. This failure does
not depend on hash iteration order.

The Alpha definition selects every maximal pair from the set of valid pairs:
see Definition 2.16, steps 4–6, in
[de Medeiros et al., Process Mining: Extending the alpha-algorithm to Mine Short Loops](https://www.vdaalst.com/publications/p221.pdf).
This finding establishes a mismatch with that algorithm, independently of
whether this particular log falls within Alpha's rediscovery guarantees.

Enumerate valid extensions without discarding alternatives, deduplicate, then
retain maximal pairs. Verify small cases against exhaustive enumeration and
add structural and token-replay tests. Budget for the combinatorial search.

### P2: valid timestamps can reverse observed causality

Location: `crates/spindle-core/src/mining.rs:87–88`;
incorrect general assurance in `docs/src/guides/mining.md:526`.

Lexical ordering is not chronological ordering for all ISO-8601 strings.
Both examples reverse the correct activity order:

```text
first:  2026-01-17T10:00:00+02:00
second: 2026-01-17T09:00:00Z

first:  2026-01-17T10:00:00Z
second: 2026-01-17T10:00:00.100Z
```

This changes start/end activities, footprints, mined nets and rule direction.
Parse supported timestamps into instants, or validate and explicitly require
a normalized fixed-width format. Define invalid-input and equal-time behavior.
The workspace already depends on `chrono`.

### P2: rule metrics ignore literal semantics

Location: `crates/spindle-core/src/mining.rs:879–892`.

`rules_with_metrics` checks arity, then extracts only names. Five `a,b` traces
give the rule `a => -b` support **5** and confidence **1.0**. The evidence
actually supports positive `b`. Negation, modes, arguments and temporal
qualifiers are not matched to observations.

Reject unsupported rule forms or use an explicit event-to-literal mapping.
Checking that a rule is unary is insufficient. The counterexample for a
negated head is executable in the probes.

### P2: the sequential-trace helper reorders long traces

Location: `crates/spindle-core/src/mining.rs:990–1002`.

The generated hour is `10 + i`, without date rollover. The fifteenth event
already has hour 24. Once the hour reaches 100, lexical sorting places it
before hour 10: a 100-event trace starts with activities 90–99, then 0–89.
All three public log helpers inherit this problem. Generate valid timestamps
using a duration from a fixed base, or preserve explicit sequence order.

## Usefulness and limits

- **Worth keeping:** direct-follow footprints, per-trace support, and candidate
  relationship discovery for human workflow exploration. Sequential toy logs
  are handled sensibly when timestamp formatting is uniform.
- **Not ready for automatic policy or compliance decisions:** activity labels
  alone cannot distinguish an observation, a future prediction, an obligation,
  or a completed task. The emitted rules erase these distinctions.
- **Conflict records are hypotheses:** mutex detection checks only absence of
  co-occurrence, despite the comment about shared predecessors at line 692.
  It does not establish impossibility or condition on case attributes. A
  structural choice is local token competition, not necessarily global
  exclusion across an entire execution.
- **Confidence has a narrow contract:** its denominator excludes cases where
  `a` is terminal. Nine complete `[a]` traces plus one `[a,b]` trace yield 1.0
  for `a -> b`, not 0.1. This agrees with the function's explicit documentation,
  so it is not listed as an implementation defect. It is unsuitable as an
  unconditional probability that a case containing `a` will contain `b`.
  The guide's occurrence-based explanation also disagrees with the current
  per-trace implementation when activities repeat.
- **Real-data fragility:** one reverse adjacency changes an otherwise frequent
  causal pair to parallel before support/confidence filtering. The thresholds
  do not make the Alpha miner noise tolerant. Short-loop limitations are
  inherent in basic Alpha; they should not be confused with the greedy-search
  implementation defect above. The cited paper describes its completeness,
  noise and short-loop assumptions.
- **Limited integration:** no production caller in the CLI or WASM bindings,
  event-log importer, token replay/conformance evaluator, or net exporter was
  found. Actors, bindings and annotations are carried but not mined. The Rust
  API requires callers to assemble logs themselves.
- **Scaling remains unverified:** footprints are rebuilt three times in the
  pipeline. Metrics rescan the log for each causal pair; mutex detection
  rebuilds per-case activity sets for each activity pair. Precompute evidence
  once before treating this as a large-log facility. No performance claim is
  justified by this review.

Recommendation: keep an explicitly experimental discovery API and separate
association candidates from executable rules. Fix chronological ordering and
Alpha enumeration first, then establish the intended semantics before adding
CLI/WASM exposure. There is useful groundwork here, but automatic rule learning
is currently overstated.

## Validation and reproduction

`make check` passes (formatting and Clippy). The existing mining unit suite
passes all 60 tests. Several tests assert only
nonempty structures; some contain tautologies or discard the value they purport
to check (for example lines 1242–1246, 1375, 1556 and 1759). They do not protect the
semantics exercised here.

The accompanying `process-mining-probes.rs` asserts desired behavior. Running
it against the reviewed revision produced **7 failures and 1 pass**: Alpha
enumeration, two timestamp cases, XOR semantics, AND-join semantics, negative
literal metrics, and long-trace ordering fail; the documented trace-count
metric control passes. The XOR/AND probes test the advertised process-to-rule
interpretation; a deliberately association-only API would instead need a
narrower documented contract.

To reproduce from the repository root (the destination must not already exist):

```sh
cp docs/reviews/process-mining-probes.rs crates/spindle-core/tests/process_mining_review.rs
cargo test -p spindle-core --test process_mining_review -- --nocapture
rm crates/spindle-core/tests/process_mining_review.rs
```

The temporary test file was removed after the review. The retained probes live
outside Cargo's normal suite because they intentionally expose unresolved
defects. No production code was changed. Before committing, `make fmt-fix`,
`make check`, and `make test` passed: 2,265 workspace tests passed (36 skipped),
and 22 doctests passed (3 ignored). Performance benchmarks were not run.
