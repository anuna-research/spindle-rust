# Variables and Grounding

Spindle supports first-order variables using Datalog-style bottom-up grounding.

## Variable Syntax

Variables are prefixed with `?`:

```lisp
?x
?person
?any_value
```

## Basic Example

```lisp
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

```lisp
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

## How Grounding Works

### Step 1: Collect Ground Facts

Extract all predicate instances from facts:

```lisp
(given (parent alice bob))   →  parent(alice, bob)
(given (parent bob charlie)) →  parent(bob, charlie)
```

### Step 2: Match Rule Bodies

For each rule, find all substitutions that satisfy the body:

```lisp
(normally r1 (parent ?x ?y) (ancestor ?x ?y))
```

Substitutions:
- `{?x → alice, ?y → bob}`
- `{?x → bob, ?y → charlie}`

### Step 3: Generate Ground Rules

Apply substitutions to create ground instances:

```lisp
; Ground instances of r1
(normally r1_1 (parent alice bob) (ancestor alice bob))
(normally r1_2 (parent bob charlie) (ancestor bob charlie))
```

### Step 4: Forward Chaining

Reason over the ground theory using standard algorithms.

## Multiple Variables

```lisp
(given (edge a b))
(given (edge b c))
(given (edge c d))

(normally path (and (edge ?x ?y) (edge ?y ?z)) (connected ?x ?z))
```

The join `(edge ?x ?y)` ∧ `(edge ?y ?z)` requires matching on `?y`:

- `{?x→a, ?y→b}` joins with `{?y→b, ?z→c}` → `connected(a, c)`
- `{?x→b, ?y→c}` joins with `{?y→c, ?z→d}` → `connected(b, d)`

## Wildcard Variable

Use `_` when you don't care about a value:

```lisp
(normally r1 (parent _ ?y) (has-parent ?y))
```

This matches any parent relationship.

## Variable Scope

Variables are scoped to their rule:

```lisp
; ?x in r1 is independent of ?x in r2
(normally r1 (parent ?x ?y) (ancestor ?x ?y))
(normally r2 (friend ?x ?y) (knows ?x ?y))
```

## Safety Requirement

All head variables must appear in the body (**range-restricted**):

```lisp
; VALID: ?x and ?y appear in body
(normally r1 (parent ?x ?y) (ancestor ?x ?y))

; INVALID: ?z not in body (unsafe)
(normally r2 (parent ?x ?y) (triple ?x ?y ?z))
```

Unsafe rules would generate infinite ground instances.

## Negation with Variables

Negated predicates in the body:

```lisp
(given (bird tweety))
(given (penguin tweety))
(given (bird eddie))

; Non-penguin birds fly
(normally r1 (and (bird ?x) (not (penguin ?x))) (flies ?x))
```

**Important**: The negated predicate must be **ground** after substitution. Spindle uses stratified negation.

## Grounding with Superiority

Superiority applies to the **rule template**, affecting all ground instances:

```lisp
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

Tips:
- Keep rule bodies small
- Use specific predicates to reduce matching
- Add explicit superiority where grounded conflicts should resolve to one side

## Variables vs. Manual Enumeration

SPL supports variables, so you can write a single rule:

```lisp
; Single rule with variables
(normally r1 (parent ?x ?y) (ancestor ?x ?y))
```

Without variables, you would need to enumerate all ground instances manually:

```lisp
(normally r1 (parent alice bob) (ancestor alice bob))
(normally r2 (parent bob charlie) (ancestor bob charlie))
```
