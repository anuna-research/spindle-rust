# Algorithms

Spindle implements two reasoning algorithms: standard DL(d) and scalable DL(d||).

## Standard DL(d)

The default algorithm uses **forward chaining** to compute conclusions.

### How It Works

1. **Initialize**: Add all facts to the proven set
2. **Propagate**: Find rules whose bodies are satisfied
3. **Fire**: Add conclusions, checking for conflicts
4. **Repeat**: Until no new conclusions

### Pseudocode

```
function reason(theory):
    proven = {}

    # Phase 1: Facts
    for fact in theory.facts:
        add_conclusion(+D, fact.head)
        add_conclusion(+d, fact.head)
        worklist.add(fact.head)

    # Phase 2: Forward chaining
    while worklist not empty:
        literal = worklist.pop()

        for rule in rules_with_body_containing(literal):
            if body_satisfied(rule, proven):
                if rule.type == Strict:
                    add_conclusion(+D, rule.head)
                    add_conclusion(+d, rule.head)
                elif rule.type == Defeasible:
                    if not blocked_by_superior(rule):
                        add_conclusion(+d, rule.head)
                # Defeaters don't add conclusions

    # Phase 3: Negative conclusions
    for literal in all_literals:
        if literal not in proven[+D]:
            add_conclusion(-D, literal)
        if literal not in proven[+d]:
            add_conclusion(-d, literal)
```

### Conflict Resolution

When a defeasible rule fires, we check for blocking:

```
function blocked_by_superior(rule):
    complement = negate(rule.head)

    for attacker in rules_for(complement):
        if body_satisfied(attacker):
            if attacker.type == Defeater:
                if not is_superior(rule, attacker):
                    return true  # Blocked by defeater
            elif attacker.type == Defeasible:
                if is_superior(attacker, rule):
                    return true  # Blocked by superior rule

    return false
```

### Complexity

- Time: O(n * m) where n = rules, m = average body size
- Space: O(n) for proven literals

## Scalable DL(d||)

For large theories, the three-phase algorithm is more efficient.

### Overview

```
Theory → [Delta] → [Lambda] → [Partial] → Conclusions
            ↓          ↓           ↓
       Definite   Potential   Defeasible
       (+D)       (approx)    (+d)
```

### Phase 1: Delta Closure (P_Δ)

Computes **definite conclusions** using only facts and strict rules.

```
function compute_delta(theory):
    delta = {}
    worklist = theory.facts

    while worklist not empty:
        literal = worklist.pop()
        delta.add(literal)

        for rule in strict_rules_with_body(literal):
            if body_satisfied(rule, delta):
                if rule.head not in delta:
                    worklist.add(rule.head)

    return delta
```

**Result**: All `+D` conclusions.

### Phase 2: Lambda Closure (P_λ)

Computes an **over-approximation** of defeasible conclusions.

A literal is in lambda if:
1. It's in delta, OR
2. Some strict/defeasible rule has:
   - Body fully in lambda
   - Complement NOT in delta

```
function compute_lambda(theory, delta):
    lambda = delta.copy()
    worklist = delta.copy()

    while worklist not empty:
        literal = worklist.pop()

        for rule in rules_with_body(literal):
            if rule.type in [Strict, Defeasible]:
                if body_satisfied(rule, lambda):
                    if complement(rule.head) not in delta:
                        if rule.head not in lambda:
                            lambda.add(rule.head)
                            worklist.add(rule.head)

    return lambda
```

**Key insight**: If something is NOT in lambda, it's definitely not defeasibly provable. This allows efficient pruning.

### Phase 3: Partial Closure (P_∂||)

Computes actual **defeasible conclusions** with conflict resolution.

```
function compute_partial(theory, delta, lambda):
    partial = delta.copy()

    candidates = lambda - delta  # Only check these
    changed = true

    while changed:
        changed = false
        for literal in candidates:
            if literal not in partial:
                if can_prove_defeasibly(literal, partial, lambda):
                    partial.add(literal)
                    changed = true

    return partial
```

The `can_prove_defeasibly` function:

```
function can_prove_defeasibly(literal, partial, lambda):
    # Need a supporting rule with satisfied body
    has_support = false
    for rule in rules_for(literal):
        if rule.type in [Strict, Defeasible]:
            if body_satisfied(rule, partial):
                has_support = true
                break

    if not has_support:
        return false

    # All attacks must be defeated
    for attacker in rules_for(complement(literal)):
        # Attack fails if body not satisfiable
        if not body_satisfiable(attacker, lambda):
            continue

        # Attack fails if we have a superior defender
        if team_defeats(literal, attacker, partial):
            continue

        # This attack succeeds
        return false

    return true
```

### Performance Comparison

| Aspect | Standard | Scalable |
|--------|----------|----------|
| Small theories (<100 rules) | Faster | Overhead |
| Large theories (>1000 rules) | Slower | Faster |
| Memory | Lower | Higher (3 sets) |
| Conflicts | Per-rule check | Batch processing |

### When to Use Scalable

Use `--scalable` when:
- Theory has >1000 rules
- Many potential conflicts
- Long inference chains
- Lambda pruning will help

## Semi-Naive Evaluation

Phase 3 uses **semi-naive evaluation** for efficiency:

Instead of checking all candidates each iteration, only check literals whose dependencies changed:

```
function compute_partial_seminaive(theory, delta, lambda):
    partial = delta.copy()
    worklist = rules_with_empty_body()

    while worklist not empty:
        literal = worklist.pop()

        if can_prove_defeasibly(literal, partial, lambda):
            partial.add(literal)

            # Only add rules triggered by this literal
            for rule in rules_with_body(literal):
                if body_satisfied(rule, partial):
                    worklist.add(rule.head)

    return partial
```

This avoids redundant checks and provides significant speedup for large theories.

## Semantic Equivalence

Both algorithms produce identical results for well-formed theories:

```
∀ theory: reason(theory) ≡ reason_scalable(theory)
```

The test suite verifies this property across many test cases.
