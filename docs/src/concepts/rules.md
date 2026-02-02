# Rules and Facts

Spindle supports four types of rules, each with different semantics for how conclusions are drawn.

## Facts (`>>`)

Facts are unconditional truths. They have no body (antecedent) and are always true.

**DFL:**
```dfl
f1: >> bird
f2: >> penguin
f3: >> -guilty    # Negated fact
```

**SPL:**
```lisp
(given bird)
(given penguin)
(given (not guilty))
```

Facts produce **definite conclusions** (`+D`) that cannot be defeated.

## Strict Rules (`->`)

Strict rules express necessary implications. If the body is true, the head **must** be true.

**DFL:**
```dfl
r1: penguin -> bird        # All penguins are birds
r2: human, mortal -> dies  # All mortal humans die
```

**SPL:**
```lisp
(always r1 penguin bird)
(always r2 (and human mortal) dies)
```

Strict rules produce **definite conclusions** (`+D`). They cannot be defeated by defeasible rules.

### When to Use Strict Rules

Use strict rules for:
- Definitional relationships (`penguin -> bird`)
- Logical necessities (`p, p->q -> q`)
- Constraints that have no exceptions

## Defeasible Rules (`=>`)

Defeasible rules express typical or default behavior that may have exceptions.

**DFL:**
```dfl
r1: bird => flies           # Birds typically fly
r2: student => has_loans    # Students typically have loans
```

**SPL:**
```lisp
(normally r1 bird flies)
(normally r2 student has_loans)
```

Defeasible rules produce **defeasible conclusions** (`+d`) that can be defeated by:
- Strict rules proving the opposite
- Superior defeasible rules
- Defeaters

### When to Use Defeasible Rules

Use defeasible rules for:
- Default behaviors with exceptions
- Typical properties
- Rules of thumb

## Defeaters (`~>`)

Defeaters are special rules that **block** conclusions without proving anything themselves.

**DFL:**
```dfl
d1: broken_wing ~> flies    # A broken wing blocks "flies"
```

**SPL:**
```lisp
(except d1 broken_wing flies)
```

### Defeater vs. Defeasible Rule

```dfl
# Defeasible rule: proves -flies
r1: penguin => -flies

# Defeater: only blocks flies, doesn't prove -flies
d1: sick ~> flies
```

The difference:
- `r1` can prove `-flies` if its body is satisfied
- `d1` can only block `flies`, it never proves `-flies`

### When to Use Defeaters

Use defeaters when:
- You want to express doubt without asserting the opposite
- Evidence should block a conclusion but not prove its negation
- You're modeling uncertainty

## Rule Bodies (Antecedents)

Rule bodies can contain:

### Single Literal
```dfl
r1: bird => flies
```

### Multiple Literals (Conjunction)
```dfl
r1: bird, healthy => flies
r2: student, employed => busy
```

**SPL:**
```lisp
(normally r1 (and bird healthy) flies)
```

### Negated Literals
```dfl
r1: bird, -penguin => flies   # Non-penguin birds fly
```

## Rule Heads (Consequents)

Rule heads are single literals that can be:

### Positive
```dfl
r1: bird => flies
```

### Negated
```dfl
r1: penguin => -flies
```

## Rule Labels

Every rule has a label (identifier) used for:
- Superiority relations (`r2 > r1`)
- Explanations
- Debugging

**DFL:** Labels are required and appear before the colon:
```dfl
my_rule: bird => flies
```

**SPL:** Labels are optional (auto-generated if omitted):
```lisp
(normally r1 bird flies)      ; labeled r1
(normally bird flies)         ; auto-labeled
```

## Summary

| Rule Type | Symbol | Conclusion | Can be Defeated? |
|-----------|--------|------------|------------------|
| Fact | `>>` | +D | No |
| Strict | `->` | +D | No |
| Defeasible | `=>` | +d | Yes |
| Defeater | `~>` | None (blocks only) | N/A |
