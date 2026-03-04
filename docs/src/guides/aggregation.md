# Aggregation with Fold

Spindle's `fold` construct aggregates values across all facts matching a pattern, producing a single result per group. It supports common aggregation patterns like sum, count, min, and max, and can be extended with custom reducers.

## Basic Syntax

A fold appears in a rule body alongside other literals and constraints:

```lisp
(fold ?result identity reducer extract pattern)
```

| Component | Role | Examples |
|-----------|------|---------|
| `?result` | Variable receiving the aggregated value | `?total`, `?n` |
| identity | Starting value, or `required` | `0`, `1`, `required` |
| reducer | Binary function combining values | `+`, `min`, `max` |
| extract | Expression evaluated per match | `?pay`, `1`, `(* ?h ?r)` |
| pattern | Literal matched against facts | `(pay-line ?emp ?pay)` |

### Simple Example: Sum

```lisp
(given (pay-line alice 25))
(given (pay-line alice 30))
(given (pay-line bob 40))

(normally r-total
    (fold ?total 0 + ?pay (pay-line ?emp ?pay))
    (total-pay ?emp ?total))
```

For each employee, the fold:

1. Finds all `pay-line` facts matching the grouping (here, `?emp`)
2. Extracts `?pay` from each match
3. Combines extracted values with `+`, starting from the identity `0`

Results: `(total-pay alice 55)`, `(total-pay bob 40)`.

## Common Patterns

### Sum

```lisp
(fold ?total 0 + ?amount (invoice ?dept ?amount))
```

Identity `0` — returns zero for departments with no invoices.

### Count

```lisp
(fold ?n 0 + 1 (shift ?emp ?day))
```

The extract `1` contributes a constant per match; `+` sums them into a count.

### Minimum / Maximum

```lisp
(fold ?lowest required min ?rate (rate ?region ?rate))
(fold ?highest required max ?score (exam ?student ?score))
```

Use `required` as the identity — there is no sensible minimum or maximum of an empty set.

### Computed Extract

The extract can be any arithmetic expression:

```lisp
(normally r-total
    (fold ?total 0 + (* ?hours ?rate) (work ?emp ?hours ?rate))
    (daily-pay ?emp ?total))
```

Each matching `work` fact contributes `hours * rate` to the sum.

## Identity vs. Required

The identity controls what happens when no facts match the pattern.

| Identity | No-match behaviour | Use for |
|----------|-------------------|---------|
| `0` | Result is `0`, rule fires | Sum, count |
| `1` | Result is `1`, rule fires | Product |
| `required` | Rule does not fire | Min, max |

**With numeric identity:**

```lisp
(fold ?n 0 + 1 (task ?emp ?t))
```

If there are no `task` facts for an employee, `?n` binds to `0` and the rule fires.

**With `required`:**

```lisp
(fold ?m required min ?score (exam ?student ?score))
```

If there are no `exam` facts for a student, the fold fails and the rule does not fire for that student.

## Variable Scoping

### Grouping Variables

Variables that appear both *inside* the fold pattern and *outside* the fold (in other body literals or the rule head) act as **grouping variables**. The fold produces one result per distinct combination of grouping variable values.

```lisp
(given (employee alice))
(given (employee bob))
(given (pay-line alice 25))
(given (pay-line alice 30))
(given (pay-line bob 40))

(normally r-total
    (and (employee ?emp)
         (fold ?total 0 + ?pay (pay-line ?emp ?pay)))
    (total-pay ?emp ?total))
```

Here `?emp` is a grouping variable — it appears in the fold pattern `(pay-line ?emp ?pay)` and in the outer body `(employee ?emp)`. The fold runs separately for each employee:

- `alice`: `25 + 30 = 55`
- `bob`: `40`

Results: `(total-pay alice 55)`, `(total-pay bob 40)`.

### Result Variable

The `?result` variable is bound by the fold and can be used in the rule head or subsequent body elements. It must not collide with any variable bound in the fold pattern — this prevents ambiguity between the accumulated result and per-match bindings.

```lisp
; INVALID — ?pay appears in both result and pattern
(fold ?pay 0 + ?pay (pay-line ?emp ?pay))

; CORRECT
(fold ?total 0 + ?pay (pay-line ?emp ?pay))
```

## Stratification

### What is Stratification?

When a fold aggregates over a *derived* relation (one produced by other rules, rather than given as facts), the fold must wait for those rules to finish before it can run. **Stratification** partitions the rules into ordered layers called strata, ensuring each fold sees complete inputs.

### How It Works

Spindle detects fold dependencies automatically:

1. For each fold, identify the relation it aggregates over
2. If that relation is derived by other rules, the fold must be in a later stratum
3. Assign strata by topological sort of the dependency graph

Programs without folds (or where folds only aggregate over base facts) produce a single stratum and behave exactly as before.

### Example: Fold Over Derived Relations

```lisp
; Stratum 0: derive line totals
(given (work alice mon 8 25))
(given (work alice tue 6 25))
(given (work bob mon 8 30))

(normally r-line
    (and (work ?emp ?day ?hours ?rate)
         (bind ?pay (* ?hours ?rate)))
    (pay-line ?emp ?pay))

; Stratum 1: aggregate derived pay-line
(normally r-total
    (fold ?total 0 + ?pay (pay-line ?emp ?pay))
    (total-pay ?emp ?total))
```

Spindle places `r-line` in stratum 0 and `r-total` in stratum 1. All `pay-line` conclusions are computed before the fold runs, so `r-total` sees complete data.

### Transparent for Simple Programs

If your folds only aggregate over `given` facts, stratification has no visible effect — everything runs in a single stratum.

### Cycle Detection

If the dependency graph contains a cycle (rule A folds over a relation derived by rule B, and rule B folds over a relation derived by rule A), Spindle reports an error:

```
Aggregation cycle detected involving relation 'foo'.
Cycles through aggregation are not supported.
Hint: restructure so that aggregation dependencies flow in one direction.
```

### Multi-Stratum Chains

Dependencies can chain through more than two strata:

```lisp
; Stratum 0: base facts
(given (sale east 100))
(given (sale west 200))

; Stratum 1: region totals (fold over base facts -> stays stratum 0, actually)
(normally r-region
    (fold ?total 0 + ?amount (sale ?region ?amount))
    (region-total ?region ?total))

; Stratum 2: grand total (fold over derived region-total)
(normally r-grand
    (fold ?grand 0 + ?t (region-total ?r ?t))
    (grand-total ?grand))
```

Each stratum completes fully before the next begins.

## Fold with Defeasible Reasoning

Fold interacts naturally with Spindle's defeasible logic:

### Superiority Within a Stratum

Rules within the same stratum follow normal superiority and defeat. A fold aggregates over whatever conclusions survive the defeasible reasoning process in earlier strata.

### Defeat Across Strata

A conclusion produced in stratum 0 can be defeated by a rule in stratum 0 before stratum 1 sees it. The fold in stratum 1 only aggregates over the surviving conclusions.

### Fold with Strict Rules

Fold works with all rule types — `always`, `normally`, and `except`. A fold in a strict rule produces definite (`+D`) conclusions; in a defeasible rule, it produces defeasible (`+d`) conclusions.

## Multiple Folds in One Rule

A rule can contain multiple folds in an `and` body:

```lisp
(normally r-summary
    (and (employee ?emp)
         (fold ?total_hours 0 + ?h (hours ?emp ?h))
         (fold ?total_bonus 0 + ?b (bonus ?emp ?b)))
    (summary ?emp ?total_hours ?total_bonus))
```

Each fold is evaluated independently; both results are available in the head.

## Custom Reducers

The reducer can be any registered binary function — not just built-in operators. You can define custom reducers via the Rust extension function API:

```lisp
; Using a custom "mul" reducer for product
(fold ?product 1 mul ?v (value ?emp ?v))
```

See the [Rust Library guide](../integration/rust.md#extension-functions) for how to register custom functions.

## Restrictions

- **Body only** — fold cannot appear in rule heads or facts
- **Positive pattern** — the fold pattern must not be negated (`not` or `~`)
- **Result variable isolation** — `?result` must not collide with variables bound in the fold pattern
- **Binary reducer** — the reducer function must accept exactly 2 arguments
- **No temporal mixing** — programs that use temporal features (`during`) cannot have multi-stratum folds
- **Reducer must exist** — the reducer name must be a registered function (built-in or extension)
