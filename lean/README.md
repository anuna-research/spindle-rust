# SpindleLean

Lean models, proofs, and executable differential-test oracles for Spindle.

The standard, SDL/family, and aggregate oracles use traditional ambiguity-blocking
DL(∂), with four constructive tags. The aggregate backend has proved fixed-point
completion and prefix equivalence for all four tags, and its lowering proofs use
that backend. Every compared Lean/Rust result must agree. The older lambda-only
and strengthened two-sided models remain for their historical property proofs;
those theorems should not be mistaken for proofs about the new standard backend.
These proofs do not establish whole-language Rust conformance.

```sh
scripts/check-lean-verification.sh
```

The gate builds the default libraries and all oracle executables with warnings
as errors, rejects admitted proofs and local axioms, runs the vacuity checks,
and audits theorem dependencies against Lean's standard axiom whitelist.
The checked sources contain no admitted proofs. See [PROOFS.md](PROOFS.md) for
individual theorem statements and their hypotheses.

For development, from `lean/`:

```sh
lake build                  # Libraries and axiom audit
lake build AggregationOracle
lake exe spindlelean        # Default example
```

`lake exe cache get` fetches Mathlib build artifacts on a fresh checkout.

## Models and proof modules

| Location | Purpose |
| --- | --- |
| `SpindleLean/Basic.lean`, `Rule.lean`, `Theory.lean` | Ordinary literals, rules, priorities, and finite literal universe |
| `SpindleLean/Closure/` | Three-phase delta, lambda, and partial closures |
| `SpindleLean/Properties/` | Soundness, conditional consistency/containment, finite convergence, and related properties |
| `SpindleLean/FamilyTwoSided.lean` | Historical strengthened two-sided model and property proofs |
| `Spindle/Arith/` | Grounding, arithmetic, temporal, and query models |
| `Spindle/Aggregation/` | Source aggregate semantics, stratum inference, lowering correctness, and completed-prefix equivalence |
| `AxiomAudit.lean` | Dependency audit of the principal results |

The ordinary closure iteration budget is derived from the finite literal
universe (`allLiterals.length + 1`); it is not a fixed 1,000-step limit.
Containment and faithfulness results carry their stated consistency or other
hypotheses. The aggregation guide describes the supported fragment in detail.

## Aggregation oracle

`AggregationOracle` invokes the verified `evaluateProgram` entry point with
**defeasible snapshot evidence** as the default policy. It accepts typed JSON
schemas and an explicit finite domain, then returns the inferred stage count
and all four proof tags for structured user atoms. Internal closure guards are
omitted; errors remain per-case results in a batch.

The wire format and executable inputs are defined by
[AggregationOracle.lean](Spindle/DiffTest/AggregationOracle.lean) and the
[Rust differential fixtures](../crates/spindle-core/tests/lean_aggregation_oracle_difftest.rs).
This adapter is test infrastructure, not a verified SPL parser.

From the repository root:

```sh
(cd lean && lake build AggregationOracle)
cargo test -p spindle-core --test lean_aggregation_oracle_difftest -- --ignored --nocapture
```

The suite contains 86 typed cases, 26 parsed SPL pipeline cases, 512 generated
conflict/cycle/priority cases, the constructive-discard regression, and three
traditional cycle/strict-inconsistency cases. Every
case requires agreement; the former expected discrepancy now requires both
sides to derive `q` and count one row. Rust uses checked i64 arithmetic; Lean
aggregates use exact Int, so agreement cases stay within safe arithmetic bounds.

## Other differential suites

Build their executables through the full verification script, then run the
external-oracle tests explicitly:

```sh
cargo test -p spindle-core --test lean_sdl_exhaustive_difftest -- --ignored
cargo test -p spindle-core --test lean_family_exhaustive_difftest -- --ignored
cargo test -p spindle-core --test lean_arith_oracle_difftest -- --ignored
cargo test -p spindle-core --test lean_grounding_oracle_difftest -- --ignored
cargo test -p spindle-core --test lean_trust_oracle_difftest -- --ignored
```

[Forgejo CI](../.forgejo/workflows/ci.yml) runs the proof gate and aggregate
comparison. [Woodpecker CI](../.woodpecker/ci.yaml) additionally lists the other
oracle suites.
