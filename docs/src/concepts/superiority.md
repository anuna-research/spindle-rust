# Superiority

Superiority relations resolve conflicts between competing rules.

## The Problem

When two rules conclude opposite things, we have a conflict:

```lisp
f1: >> bird
f2: >> penguin
r1: bird => flies
r2: penguin => -flies
```

Both r1 and r2 fire. Without superiority, we get **ambiguity** - neither `flies` nor `-flies` is provable.

## Declaring Superiority

The `>` operator declares that one rule is superior to another:

```lisp
r2 > r1    # r2 beats r1
```

Now when both rules fire, r2 wins and `-flies` is provable.

**SPL:**
```lisp
(prefer r2 r1)
```

## Superiority Chains

You can declare chains of superiority:

**SPL:**
```lisp
r3 > r2
r2 > r1
```

**SPL (shorthand):**
```lisp
(prefer r3 r2 r1)   ; r3 > r2 > r1
```

## Transitivity

Superiority is **not automatically transitive**. If you need `r3 > r1`, declare it explicitly:

```lisp
r3 > r2
r2 > r1
r3 > r1    # Must be explicit
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
f1: >> a
f2: >> b
f3: >> c

r1: a => result
r2: b => -result
r3: c => result

r1 > r2    # r1 beats r2
r3 > r2    # r3 beats r2
```

Analysis:
- r2 attacks `result`
- Both r1 and r3 are superior to r2
- The attack is defeated
- `+d result`

## Symmetric Conflicts

If two rules are equally superior over each other (or neither is superior), ambiguity results:

```lisp
f1: >> trigger
r1: trigger => a
r2: trigger => -a
# No superiority
```

Result: Neither `a` nor `-a` is provable.

## Defeating Defeaters

Defeaters can be overridden by superiority:

```lisp
f1: >> bird
f2: >> healthy

r1: bird => flies
d1: bird ~> flies      # Defeater blocks flies
r2: healthy => flies

r2 > d1                # Healthy birds overcome the defeater
```

If both `bird` and `healthy` are true, r2 beats d1 and `flies` is provable.

## Strict Rules and Superiority

Strict rules **always win** over defeasible rules, regardless of superiority:

```lisp
f1: >> p
r1: p -> q           # Strict
r2: p => -q          # Defeasible
r2 > r1              # This has no effect!
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
r2: penguin => -flies
r1: bird => flies
r2 > r1    # Penguin is more specific than bird
```

### Document Reasoning
Use comments to explain why one rule beats another:
```lisp
# Medical override: confirmed diagnosis beats symptoms
r_diagnosis > r_symptoms
```

### Avoid Cycles
Don't create circular superiority:
```lisp
# BAD - creates a cycle
r1 > r2
r2 > r1
```

This leads to undefined behavior.
