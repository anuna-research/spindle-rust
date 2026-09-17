# Variables and Grounding

Spindle supports first-order variables using Datalog-style bottom-up grounding.

## Variable Syntax

Variables are prefixed with `?`:

```spl
?x
?person
?any_value
```

## Basic Example

```spl
; Facts with predicates
(given (parent alice bob))
(given (parent bob charlie))

; Rule with variables
(normally r1 (parent ?x ?y) (ancestor ?x ?y))
```

The rule `r1` matches against facts:
- `(parent alice bob)` → `(ancestor alice bob)`
- `(parent bob charlie)` → `(ancestor bob charlie)`

## Transitive Closure

A classic example - computing ancestors:

```spl
; Base facts
(given (parent alice bob))
(given (parent bob charlie))
(given (parent charlie david))

; Base case: parents are ancestors
(normally r1 (parent ?x ?y) (ancestor ?x ?y))

; Recursive case: ancestor of ancestor
(normally r2 (and (parent ?x ?y) (ancestor ?y ?z)) (ancestor ?x ?z))
```

**Results:**
- `ancestor(alice, bob)` - via r1
- `ancestor(bob, charlie)` - via r1
- `ancestor(charlie, david)` - via r1
- `ancestor(alice, charlie)` - via r2
- `ancestor(bob, david)` - via r2
- `ancestor(alice, david)` - via r2

## Grounding Stages

### Ground Facts

The grounder extracts all predicate instances from facts:

```spl
(given (parent alice bob))   →  parent(alice, bob)
(given (parent bob charlie)) →  parent(bob, charlie)
```

### Rule Body Matching

For each rule, the grounder finds all substitutions that satisfy the body:

```spl
(normally r1 (parent ?x ?y) (ancestor ?x ?y))
```

Substitutions:
- `{?x → alice, ?y → bob}`
- `{?x → bob, ?y → charlie}`

### Ground Rule Generation

The grounder applies substitutions to create ground instances:

```spl
; Ground instances of r1
(normally r1_1 (parent alice bob) (ancestor alice bob))
(normally r1_2 (parent bob charlie) (ancestor bob charlie))
```

### Forward Chaining

Standard algorithms reason over the ground theory.

## Multiple Variables

```spl
(given (edge a b))
(given (edge b c))
(given (edge c d))

(normally path (and (edge ?x ?y) (edge ?y ?z)) (connected ?x ?z))
```

The join `(edge ?x ?y)` ∧ `(edge ?y ?z)` requires matching on `?y`:

- `{?x→a, ?y→b}` joins with `{?y→b, ?z→c}` → `connected(a, c)`
- `{?x→b, ?y→c}` joins with `{?y→c, ?z→d}` → `connected(b, d)`

## Wildcard Variable

The wildcard `_` matches any value:

```spl
(normally r1 (parent _ ?y) (has-parent ?y))
```

This matches any parent relationship.

## Variable Scope

Each rule has its own variable scope:

```spl
; ?x in r1 is independent of ?x in r2
(normally r1 (parent ?x ?y) (ancestor ?x ?y))
(normally r2 (friend ?x ?y) (knows ?x ?y))
```

## Safety Requirement

All head variables appear in the body of a safe rule (**range-restricted**):

```spl
; VALID: ?x and ?y appear in body
(normally r1 (parent ?x ?y) (ancestor ?x ?y))

; INVALID: ?z not in body (unsafe)
(normally r2 (parent ?x ?y) (triple ?x ?y ?z))
```

Unsafe rules generate infinite ground instances.

## Negation with Variables

Negated predicates in the body:

```spl
(given (bird tweety))
(given (penguin tweety))
(given (bird eddie))

; Non-penguin birds fly
(normally r1 (and (bird ?x) (not (penguin ?x))) (flies ?x))
```

**Important**: This is strong negation, not negation-as-failure. The rule needs
explicit support for `(not (penguin ?x))`; merely omitting a penguin fact does
not make Eddie fly. The fact `(given (not (penguin eddie)))` supplies that premise.
Stratification handles aggregate dependencies. It does not turn ordinary negation
into a test for missing evidence.

## Grounding with Superiority

Superiority applies to the **rule template**, affecting all ground instances:

```spl
(given (bird tweety))
(given (penguin tweety))

(normally r1 (bird ?x) (flies ?x))
(normally r2 (penguin ?x) (not (flies ?x)))
(prefer r2 r1)
```

Result: `r2` beats `r1` for all matching instances, so `¬flies(tweety)`.

## Performance Considerations

Grounding can produce many rules:

| Facts | Rule Body Size | Ground Rules |
|-------|----------------|--------------|
| 100 | 1 | 100 |
| 100 | 2 (join) | up to 10,000 |
| 100 | 3 (join) | up to 1,000,000 |

Small rule bodies and specific predicates reduce matching.
Explicit superiority resolves grounded conflicts in favor of one side.

## Arithmetic in Grounded Rules

After substituting variables, the grounder evaluates arithmetic expressions.

### Bind Constraints

```spl
(given (item widget 25))
(given (item gadget 10))
(given (tax-rate 0.1))

(normally r1
  (and (item ?name ?price) (tax-rate ?rate)
       (bind ?total (+ ?price (* ?price ?rate))))
  (total-cost ?name ?total))
```

Grounding:
1. The grounder matches `(item widget 25)` and `(tax-rate 0.1)` → `{?name→widget, ?price→25, ?rate→0.1}`
2. The grounder evaluates `(+ 25 (* 25 0.1))` → `27.5`
3. The grounder binds `?total → 27.5`
4. The grounder produces `(total-cost widget 27.5)`

### Comparison Guards

Guards filter substitutions that don't satisfy the comparison:

```spl
(given (score alice 85))
(given (score bob 42))

(normally r1
  (and (score ?name ?s) (>= ?s 50))
  (passing ?name))
```

Only `{?name→alice, ?s→85}` satisfies `(>= 85 50)`, so only `(passing alice)` is derived.

### Evaluation Order

The grounder evaluates body literals left-to-right. Arithmetic expressions need existing variable bindings:

```spl
; CORRECT: ?price is bound by (item ...) before (bind ...) uses it
(normally r1
  (and (item ?name ?price)
       (bind ?discounted (* ?price 0.9)))
  (sale-price ?name ?discounted))

; INCORRECT: ?price is not yet bound when (bind ...) tries to use it
(normally r-bad
  (and (bind ?discounted (* ?price 0.9))
       (item ?name ?price))
  (sale-price ?name ?discounted))
```

### Cross-Type Matching

Numeric terms match across types when values are equal:

```spl
(given (threshold 100))        ; Integer 100
(given (score alice 100.0))    ; Decimal 100.0

(normally r1
  (and (score ?name ?s) (threshold ?t) (>= ?s ?t))
  (above-threshold ?name))
```

`Integer(100)` matches `Decimal(100.0)` in comparisons, so this works as expected.

## Variables vs. Manual Enumeration

SPL supports variables, so you can write a single rule:

```spl
; Single rule with variables
(normally r1 (parent ?x ?y) (ancestor ?x ?y))
```

Without variables, each ground instance needs a separate rule:

```spl
(normally r1 (parent alice bob) (ancestor alice bob))
(normally r2 (parent bob charlie) (ancestor bob charlie))
```
