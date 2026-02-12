# Superiority

Superiority relations resolve conflicts between competing rules.

## The Problem

When two rules conclude opposite things, we have a conflict:

```lisp
(given bird)
(given penguin)
(normally r1 bird flies)
(normally r2 penguin (not flies))
```

Both r1 and r2 fire. Without superiority, we get **ambiguity** — neither `flies` nor `(not flies)` is provable.

## Declaring Superiority

```lisp
(prefer r2 r1)    ; r2 beats r1
```

Now when both rules fire, r2 wins and `(not flies)` is provable.

## Superiority Chains

Shorthand for multiple superiority relations:

```lisp
(prefer r3 r2 r1)   ; r3 > r2 > r1
```

Expands to:
```lisp
(prefer r3 r2)
(prefer r2 r1)
```

## Transitivity

Superiority is **not automatically transitive**. If you need r3 > r1, declare it explicitly:

```lisp
(prefer r3 r2)
(prefer r2 r1)
(prefer r3 r1)    ; Must be explicit
```

## Conflict Resolution Algorithm

When evaluating a defeasible conclusion:

1. Find all rules that could prove the literal
2. Find all rules that could prove the complement (attackers)
3. For each attacker with satisfied body:
   - If no defender is superior to it → blocked
   - If some defender is superior → attack fails
4. If all attacks fail → conclusion is provable

## Example: Three-Way Conflict

```lisp
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

If two rules are equally superior over each other (or neither is superior), ambiguity results:

```lisp
(given trigger)
(normally r1 trigger a)
(normally r2 trigger (not a))
; No superiority
```

Result: Neither `a` nor `(not a)` is provable.

## Defeating Defeaters

Defeaters can be overridden by superiority:

```lisp
(given bird)
(given healthy)

(normally r1 bird flies)
(except d1 bird flies)          ; Defeater blocks flies
(normally r2 healthy flies)

(prefer r2 d1)                  ; Healthy birds overcome the defeater
```

If both `bird` and `healthy` are true, r2 beats d1 and `flies` is provable.

## Strict Rules and Superiority

Strict rules **always win** over defeasible rules, regardless of superiority:

```lisp
(given p)
(always r1 p q)              ; Strict
(normally r2 p (not q))      ; Defeasible
(prefer r2 r1)               ; This has no effect!
```

Result: `+D q` (strict rule wins)

Superiority only affects conflicts between:
- Defeasible rules
- Defeasible rules and defeaters
- Defeaters

## Best Practices

### Use Specificity
More specific rules should be superior:
```lisp
(normally r1 bird flies)
(normally r2 penguin (not flies))
(prefer r2 r1)    ; Penguin is more specific than bird
```

### Document Reasoning
Use comments to explain why one rule beats another:
```lisp
; Medical override: confirmed diagnosis beats symptoms
(prefer r-diagnosis r-symptoms)
```

### Avoid Cycles
Don't create circular superiority:
```lisp
; BAD — creates a cycle
(prefer r1 r2)
(prefer r2 r1)
```

This leads to undefined behavior.
