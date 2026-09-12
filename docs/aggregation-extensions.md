# Aggregation and extension functions

SPL uses `bind` for computed values and an explicit `fold` expression for reduction
over a completed relation. This implementation builds on the function registry
from [PR #26](https://git.anuna.io/anuna-research/spindle-rust/pulls/26) and the
builtin prelude from [PR #27](https://git.anuna.io/anuna-research/spindle-rust/pulls/27).

```lisp
(aggregate-domain alice bob 0 1 2 10 20)
(given (person alice))
(given (person bob))
(given (payment alice 1 10))
(given (payment alice 2 10))
(normally total
  (and (person ?person)
       (bind ?total
         (fold + ?cost
           :from (payment ?person ?id ?cost)
           :initial 0)))
  (total-payment ?person ?total))
```

Run the complete [example](../examples/aggregation.spl):

```sh
spindle reason examples/aggregation.spl --json
spindle query '(total-payment alice 20)' examples/aggregation.spl
```

## Syntax and scope

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

`bind` is a constraint, not assignment: an existing binding must agree with the
computed value. In the aggregate fragment, all outer assignments are enumerated
from the finite domain, and every static condition is checked. A false condition
does not hide an error in another expression. Supply suitable domains: arithmetic
on a symbol is an error even if another condition would reject that assignment.

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

Reducer eligibility is separate from function registration. Arbitrary pure
functions may be order-sensitive. Fold currently uses engine-controlled checked
integer sum/min/max; registering or overriding `+` does not change the fold
reducer. There is no `foldl`/`foldr` because relations supply no iteration order.

## Limits and verification

Aggregate programs require one explicit `aggregate-domain` containing ground
integers and symbols, including possible aggregate results. Finite grounding is
exponential in outer variable count. The configured `max_instances` budget bounds
atom-table and assignment enumeration; exhaustion is an error, not a partial
answer. Grounding cannot be disabled for folds.

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

The differential suite covers 86 typed and 26 SPL pipeline agreement cases. A
separate pinned counterexample records the existing difference between Rust's
constructive defeat-discard backend and the older ordinary backend used by the
aggregate Lean proof. See [the proof guide](../lean/AGGREGATION.md). Parsing and
extension registration are tested integrations, not themselves verified Lean code.
