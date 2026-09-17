# Superiority

Superiority relations resolve conflicts between competing rules.

## The Problem

When two rules conclude opposite things, we have a conflict:

```spl
(given bird)
(given penguin)
(normally r1 bird flies)
(normally r2 penguin (not flies))
```

Both r1 and r2 fire. Without superiority, we get **ambiguity** — neither `flies` nor `(not flies)` is provable.

## Declaring Superiority

```spl
(prefer r2 r1)    ; r2 beats r1
```

Now when both rules fire, r2 wins and `(not flies)` is provable.

## Superiority Chains

Shorthand for multiple superiority relations:

```spl
(prefer r3 r2 r1)   ; r3 > r2 > r1
```

Expands to:
```spl
(prefer r3 r2)
(prefer r2 r1)
```

## Transitivity

Superiority is **not automatically transitive**. The relation r3 > r1 needs its own explicit declaration:

```spl
(prefer r3 r2)
(prefer r2 r1)
(prefer r3 r1)    ; Must be explicit
```

## Conflict Resolution Algorithm

When evaluating a defeasible conclusion:

1. The engine identifies all rules that can prove the literal.
2. The engine identifies all rules that can prove the complement (attackers).
3. Each attacker with a satisfied body faces a superiority check:
   - If no defender is superior to it → blocked
   - If some defender is superior → attack fails
4. If all attacks fail → conclusion is provable

## Example: Three-Way Conflict

```spl
(given a)
(given b)
(given c)

(normally r1 a result)
(normally r2 b (not result))
(normally r3 c result)

(prefer r1 r2)    ; r1 beats r2
(prefer r3 r2)    ; r3 beats r2
```

Analysis:
- r2 attacks `result`
- Both r1 and r3 are superior to r2
- The attack is defeated
- `+d result`

## Symmetric Conflicts

If neither opposing rule has priority, ambiguity results:

```spl
(given trigger)
(normally r1 trigger a)
(normally r2 trigger (not a))
; No superiority
```

Result: Neither `a` nor `(not a)` is provable.

## Defeating Defeaters

Defeaters can be overridden by superiority:

```spl
(given bird)
(given healthy)

(normally r1 bird flies)
(except d1 bird (not flies))          ; Defeater blocks flies
(normally r2 healthy flies)

(prefer r2 d1)                  ; Healthy birds overcome the defeater
```

If both `bird` and `healthy` are true, r2 beats d1 and `flies` is provable.

## Strict Rules and Superiority

A **definite proof** wins over merely defeasible opposition, regardless of
superiority. A strict rule has this protection when its premises are definitely
proved:

```spl
(given p)
(always r1 p q)              ; Strict
(normally r2 p (not q))      ; Defeasible
(prefer r2 r1)               ; This has no effect!
```

Result: `+D q` (strict rule wins)

Superiority does not overturn definite proofs. It resolves defeasible conflicts,
including strict rules used with only defeasible premises. Common cases include:
- Defeasible rules
- Defeasible rules and defeaters
- Defeaters

## Priority Design

### Specificity
Explicit priority lets a more specific rule override a general rule:
```spl
(normally r1 bird flies)
(normally r2 penguin (not flies))
(prefer r2 r1)    ; Penguin is more specific than bird
```

### Priority Rationale
Comments explain why one rule beats another:
```spl
; Medical override: confirmed diagnosis beats symptoms
(prefer r-diagnosis r-symptoms)
```

### Cycles
Circular superiority creates a cycle:
```spl
; BAD — creates a cycle
(prefer r1 r2)
(prefer r2 r1)
```

This leads to undefined behavior.
