# Architecture

## Workspace

| Crate | Responsibility |
|---|---|
| `spindle-core` | Theory types, preparation, grounding, reasoning, queries, trust, vocabulary |
| `spindle-parser` | SPL lexer and parser |
| `spindle-cli` | Commands, input handling, JSON envelopes and diagnostics |
| `spindle-contract` | Shared JSON DTOs and schema contracts |
| `spindle-wasm` | JavaScript bindings and structured output |

## Preparation and reasoning

```text
SPL → parser → Theory → prepare → prepared Theory → indexed reasoner → conclusions
                          │
                          ├─ ordinary: validate, rewrite wildcards, ground,
                          │            validate temporal variables, optional as-of filtering
                          │
                          └─ aggregates: infer strata, ground predicates,
                                         reason completed prefixes, lower snapshots
```

`prepare(&theory, PrepareOptions)` supplies the builtin function prelude and
merges a host registry. Ordinary preparation uses composable `PipelineStage`
implementations. Aggregate theories take a snapshot-aware preparation path;
assembling only the ordinary pipeline stages does not implement aggregate semantics.

`PrepareOptions` controls grounding limits and optional temporal/trust behavior.
The aggregate path requires grounding and currently rejects temporal/trust options.
Its `max_instances` budget includes repeated grounding passes; exhaustion returns
an error rather than partial aggregate results.

## Expressions and extensions

`BodyLiteral` represents logical premises or arithmetic constraints. `ArithExpr`
contains numeric literals, variables, general values, registry-dispatched `Call`
nodes, and `Fold` nodes. `ExtensionFunction` implementations receive evaluated
`Term` arguments and return a value or error; they do not receive theory access.

Named aggregators have a separate namespace in `FunctionRegistry`. Their reducer,
optional integer identity, and row-counting flag determine fold behavior.
Custom reducers must satisfy the host contract of purity, associativity, and
commutativity. See [Aggregation and Extension Functions](../guides/aggregation.md).

## Literal identity and vocabulary

`Term` distinguishes symbols, integers, decimals, and finite floats. Reasoning
indexes use literal identity including the relevant temporal information;
`PredicateSymbol` is only the structural `(functor, arity)` identity used by
vocabulary tooling. It is not a proof-state key.

Predicate declarations, metadata, and provenance survive theory reconstruction.
`Theory::metadata()` is label-keyed; `predicate_metadata()` is a separate store.
Vocabulary derivation reports declarations, conflicts, argument profiles, and
shape diagnostics without changing reasoning.

## Constructive proof state

`StandardReasoner` implements traditional ambiguity-blocking DL(∂). State and
propagation live under `reason/`, with definite and defeasible phases maintaining
positive and negative evidence. Negative conclusions require constructive proof;
there is no final sweep that labels every unproved literal negative.

Indexes and per-slot body counters support propagation. An unresolved cyclic
premise remains undecided and cannot be treated as a disproved attacker. Definite
proof implies defeasible proof even for contradictory facts; inconsistency does
not cause unrelated conclusions. See [Algorithms](../guides/algorithms.md).

## Aggregate evidence

Aggregate consumers read completed earlier strata, retaining priorities and
attackers in the earlier rule prefix. Matching rows are deduplicated as whole
rows, then reduced. Lowering inserts fresh defeasible snapshot premises so a
strict aggregate rule does not promote defeasible row evidence to `+D`.
Internal snapshot predicates are hidden from user conclusions; prepared theories
retain them for audit, and lowered rules retain source template labels.

## Queries and trust

Queries reason over prepared theories and use exact matching for bounded temporal
goals. `requires_with_options` verifies raw abduction candidates by rerunning
reasoning. Explanation and blocker diagnostics connect grounded instances to
source templates.

The trust pass computes weakest-link credibility, applies diminishment from
applicable overruled defeaters to positive defeasible conclusions, and then
checks thresholds. Definite conclusions are exempt from diminishment.

See [Verification](verification.md) for the boundary between Lean proofs, Rust
regression tests, and differential oracle checks.
