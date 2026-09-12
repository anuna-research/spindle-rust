# Aggregation and extension functions

SPL uses a direct output-binding aggregate premise, following Skein:
`(agg ?total sum ?amount (payment ?id ?amount))`. A named aggregator defines its
combining operation and empty-input behavior. Ordinary calculations still use
`bind`. Both use the extension registry introduced by PRs #26 and #27.

```lisp
(given (person alice))
(given (person bob))
(given (payment alice 1 10))
(given (payment alice 2 10))
(normally total
  (and (person ?person)
       (agg ?total sum ?cost (payment ?person ?id ?cost)))
  (total-payment ?person ?total))
```

Run the complete [example](../examples/aggregation.spl):

```sh
spindle reason examples/aggregation.spl --json
spindle query '(total-payment alice 20)' examples/aggregation.spl
```

## Syntax and scope

```lisp
(agg ?total sum ?amount (payment ?person ?id ?amount))
(agg ?number count ?id (payment ?person ?id ?amount))
(agg ?smallest min-of ?amount (payment ?person ?id ?amount))
(agg ?largest max-of ?amount (payment ?person ?id ?amount))
```

| Name | Contribution | Empty input |
|---|---|---|
| `sum` | Selected integer value | Zero |
| `count` | One per distinct matched row; the selected variable may be a symbol | Zero |
| `min-of` | Selected integer value | Premise fails |
| `max-of` | Selected integer value | Premise fails |

The result and contribution must be variables. The contribution variable must
occur in the row pattern. The current form accepts **one row pattern**; use a
helper relation to express joins or filters. Aggregation is only valid as a rule
premise, not as a fact, head, or negated premise.

The explicit fold syntax remains supported:

```text
(bind ?result (fold reducer extraction :from pattern empty-policy))
empty-policy = :initial expression | :require-nonempty
```

Exactly one `:from` and one empty-input policy are required. Their order is
flexible. Supported reducers are `+`/`sum`, `min`, and `max`. Counting uses `1`
as the extraction. The seed contributes once, including on empty input; it need
not be an identity. `:require-nonempty` makes the condition unsatisfied on empty
input. A fold must appear directly in `bind`; use another binding or rule for
further calculations. Nested folds require separate rules/strata.

Variables occurring in ordinary premises, rule heads, other bindings/comparisons,
fold result bindings, or seed expressions belong to the outer scope. Variables
confined to a fold's row pattern are local to each matching row. Extraction
variables absent from its pattern are also outer variables. A variable shared
only between two fold patterns is local to each fold, not an implicit join.

In the example, `?person` selects a group; `?id` and `?cost` are row-local.
Distinct whole rows contribute separately even when their extracted values are
equal. Multiple proofs of an identical row do not multiply its contribution.

`agg` and `bind` introduce their result variable. If it is already bound, the
computed value must agree. Bind grouping variables in earlier ordinary premises;
row-local variables stay inside the aggregate. Head variables and expression
inputs must have a binding source. Unsafe variables are preparation errors.

Inputs can be derived predicates. For example:

```lisp
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
including symbols, and must be pure and deterministic. They have no theory-query
argument. The CLI ships the builtin prelude; this is an embedding API, not a
runtime plugin loader.

```lisp
(bind ?day (day-of-week ?date))
(bind ?total
  (fold + (adjust-cost ?cost)
    :from (payment ?person ?cost)
    :initial 0))
```

The host must register `day-of-week` and `adjust-cost` for these examples.
Unknown functions and invalid arities are validation errors. Ordinary grounding
retains its existing behavior of discarding a substitution when a function fails;
aggregate snapshot evaluation reports expression failures as errors.

`FunctionRegistry` also has a separate aggregator namespace. Register a named
`AggregatorDefinition` with `register_aggregator(name, definition)`. Its fields
are the binary `reducer` name, an optional integer `identity`, and whether to
`count` rows rather than use their selected values. The identity is both the
starting accumulator and the empty result; `None` requires a nonempty input.
Host registries can override aggregator definitions without changing ordinary
same-named functions.

The reducer can be the engine-controlled +/sum, min or max, or a registered binary
extension function returning an integer. Custom reducers must be pure,
associative and commutative over accepted inputs; registration is a host contract,
not a proof of those laws. Builtin reducer names retain their kernel meanings even
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
arithmetic inside ordinary predicate arguments (use `bind` instead). Expressions
and fold results use checked i64 arithmetic; overflow is an error. Extensions in
aggregate programs must return supported integer/symbol values. General arithmetic
outside aggregation still supports the existing numeric types.

The Lean model uses exact integers. Checked overflow can depend on intermediate
reduction order, so the implementation's deterministic row traversal is not a
machine-arithmetic refinement proof. Custom extension implementations and general
symbol bindings are also outside the current proof fragment.

Builtin named aggregates lower to the existing typed fold model. Differential
checks compare all tagged conclusions with the independent Lean evaluator,
including grouping, empty inputs, duplicate contributions, conflicts and
traditional DL(∂) cycle behavior. Every compared case must agree. Custom reducer
functions remain outside the Lean proof fragment. Parsing and extension
registration are tested integrations, not themselves verified Lean code.
See [the proof guide](../lean/AGGREGATION.md).

The domain-free aggregate operation is proved against independent unordered
predicate semantics in `Spindle/Aggregation/Binding.lean`, including agreement
with the finite reference model when its domain contains the result. The Rust
predicate grounder itself is differentially tested, not formally verified.
