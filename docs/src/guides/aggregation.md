# Aggregation and extension functions

SPL uses a direct output-binding aggregate premise, following Skein:
`(agg ?total sum ?amount (payment ?id ?amount))`. A named aggregator defines its
combining operation and empty-input behavior. Ordinary calculations still use
`bind`. Both use the extension registry introduced by PRs #26 and #27.

```spl
(given (person alice))
(given (person bob))
(given (payment alice 1 10))
(given (payment alice 2 10))
(normally total
  (and (person ?person)
       (agg ?total sum ?cost (payment ?person ?id ?cost)))
  (total-payment ?person ?total))
```

Commands for the complete [example](https://git.anuna.io/anuna-research/spindle-rust/src/branch/main/examples/aggregation.spl):

```sh
spindle reason examples/aggregation.spl --json
spindle query '(total-payment alice 20)' examples/aggregation.spl
```

## Syntax and scope

```spl
(agg ?total sum ?amount (payment ?person ?id ?amount))
(agg ?number count ?id (payment ?person ?id ?amount))
(agg ?smallest min-of ?amount (payment ?person ?id ?amount))
(agg ?largest max-of ?amount (payment ?person ?id ?amount))
```

| Name | Contribution | Empty input |
|---|---|---|
| `sum` | Selected integer value | Zero |
| `count` | One per distinct matched row; symbols are accepted as selected values | Zero |
| `min-of` | Selected integer value | Premise fails |
| `max-of` | Selected integer value | Premise fails |

The result and contribution are variables. The contribution variable occurs in the row pattern.
The current form accepts **one row pattern**. Helper relations express joins or filters. Aggregation is only valid as a rule
premise, not as a fact, head, or negated premise.

The explicit fold syntax remains supported:

```text
(bind ?result (fold reducer extraction :from pattern empty-policy))
empty-policy = :initial expression | :require-nonempty
```

The syntax requires exactly one `:from` and one empty-input policy. Their order is
flexible. Supported reducers are `+`/`sum`, `min`, and `max`. Counting uses `1`
as the extraction. The seed contributes once, including on empty input; it need
not be an identity. `:require-nonempty` makes the condition unsatisfied on empty
input. A fold appears directly in `bind`. Further calculations use another binding or rule. Nested folds require separate rules/strata.

Variables occurring in ordinary premises, rule heads, other bindings/comparisons,
fold result bindings, or seed expressions belong to the outer scope. Variables
confined to a fold's row pattern are local to each matching row. Extraction
variables absent from its pattern are also outer variables. A variable shared
only between two fold patterns is local to each fold, not an implicit join.

In the example, `?person` selects a group; `?id` and `?cost` are row-local.
Distinct whole rows contribute separately even when their extracted values are
equal. Multiple proofs of an identical row do not multiply its contribution.

`agg` and `bind` introduce their result variable. For an already bound variable, the computed value matches the existing value.
Earlier ordinary premises bind grouping variables. Row-local variables stay inside the aggregate.
Head variables and expression inputs need a binding source. Unsafe variables are preparation errors.

Inputs can be derived predicates. For example:

```spl
(given (purchase alice first 30))
(given (purchase alice second 45))
(normally eligible
  (purchase ?person ?id ?cost)
  (payment ?person ?id ?cost))
(normally total
  (agg ?total sum ?cost (payment ?person ?id ?cost))
  (all-payments ?total))
```

This derives `(all-payments 75)`. No fact or declaration enumerates `75`.

## Execution and evidence

```text
SPL -> value AST -> source schemas -> inferred strata
                                         |
                    full earlier rule prefix + priorities
                                         |
                                  ordinary reasoner
                                         |
                                  completed +D/+d rows
                                         |
                            match -> extract -> reduce
                                         |
                     lowered rules with snapshot premises
                                         |
                              normal reasoning / CLI
```

A stratum is a group of predicates at one dependency level.
Ordinary dependencies have non-strict stratum ordering; fold dependencies require
a strictly earlier stratum. Cycles through a fold are rejected. Strong negation
shares its predicate's stratum. Earlier attackers and priorities are retained.

A strict aggregate rule retains its kind but receives a fresh defeasible snapshot
premise. This prevents automatic `+D` from aggregate evidence. Independent strict
proofs can still establish the head definitely. Internal snapshot predicates are
hidden from conclusions; lowered rules retain source labels as template labels
for diagnostics. The prepared theory contains those internal premises for audit.

## Functions and reducers

`FunctionRegistry::with_prelude()` supplies arithmetic and rounding functions.
`prepare()` injects this prelude and merges `PrepareOptions::function_registry`.
Custom registry entries override same-named expression functions. Registries are
cloneable and preserve their functions through query options.

A host implements `ExtensionFunction` with a signature and
`eval(&[Term]) -> Result<Term, EvalError>`, then registers it with
`registry.register(Box::new(function))`. Functions receive evaluated values,
including symbols. The host contract requires pure, deterministic functions. They have no theory-query
argument. The CLI ships the builtin prelude; this is an embedding API, not a
runtime plugin loader.

```spl
(bind ?day (day-of-week ?date))
(bind ?total
  (fold + (adjust-cost ?cost)
    :from (payment ?person ?cost)
    :initial 0))
```

These examples depend on host registrations for `day-of-week` and `adjust-cost`.
Unknown functions and invalid arities are validation errors. Ordinary grounding
retains its existing behavior of discarding a substitution when a function fails;
aggregate snapshot evaluation reports expression failures as errors.

`FunctionRegistry` also has a separate aggregator namespace. `register_aggregator(name, definition)` registers a named `AggregatorDefinition`. Its fields
are the binary `reducer` name, an integer `identity` or `None`, and whether to
`count` rows rather than use their selected values. The identity is both the
starting accumulator and the empty result; `None` requires a nonempty input.
Host registries can override aggregator definitions without changing ordinary
same-named functions.

The reducer can be the engine-controlled +/sum, min or max, or a registered binary
extension function returning an integer. The host contract requires custom reducers to be pure, associative, and commutative over accepted inputs.
Registration does not prove those laws. Builtin reducer names retain their kernel meanings even
if an ordinary arithmetic function with that name is overridden. There is no
`foldl`/`foldr` because relations supply no user-defined iteration order.

## Limits and verification

No `aggregate-domain` is needed. The old declaration is accepted for source
compatibility but does not restrict SPL input rows or computed outputs. The typed
finite-domain reference API retains its explicit domain contract.

Grounding joins potential predicate instances; completed proofs determine the
aggregate rows. Ordinary cyclic support and attackers are retained even when
undecided. Cyclic predicates are conservatively instantiated over source and
computed constants, and completed strata are replayed when new constants appear.
This can be exponential. The configured `max_instances` budget bounds grounding
work, including repeated passes. Exhaustion (including unbounded value-generating
recursion) is an error, never a partial answer. Grounding cannot be disabled.

The aggregate bridge currently rejects decimal/float values, modal or temporal
constructs, trust-weighted snapshots, schematic predicates, wildcards, and
arithmetic inside ordinary predicate arguments (`bind` provides a separate expression binding). Expressions
and fold results use checked i64 arithmetic; overflow is an error. Extensions in aggregate programs return supported integer/symbol values. General arithmetic
outside aggregation still supports the existing numeric types.

The Lean model uses exact integers. Checked overflow can depend on intermediate
reduction order, so the implementation's deterministic row traversal is not a
machine-arithmetic refinement proof. Custom extension implementations and general
symbol bindings are also outside the current proof fragment.

Builtin named aggregates lower to the existing typed fold model. Differential
checks compare all tagged conclusions with the independent Lean evaluator,
including grouping, empty inputs, duplicate contributions, conflicts and
traditional DL(∂) cycle behavior. Any disagreement fails the differential check. Custom reducer
functions remain outside the Lean proof fragment. Parsing and extension
registration are tested integrations, not themselves verified Lean code.
See [the proof guide](https://git.anuna.io/anuna-research/spindle-rust/src/branch/main/lean/AGGREGATION.md).

`Spindle/Aggregation/Binding.lean` proves the domain-free aggregate operation against independent unordered predicate semantics.
It also proves agreement with the finite reference model when its domain contains the result. The Rust
predicate grounder itself is differentially tested, not formally verified.

## Related pages

- [Architecture](../internals/architecture.md) explains completed prefixes and snapshot evidence.
- [Benchmarking aggregation](benchmark-aggregation.md) gives the measurement workflow.
- [Benchmark measurements](../reference/aggregation-benchmarks.md) records coverage and the initial local baseline.
