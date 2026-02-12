# Rules and Facts

Spindle supports four types of rules, each with different semantics for how conclusions are drawn.

## Facts (`given`)

Facts are unconditional truths. They have no body (antecedent) and are always true.

```lisp
(given bird)
(given penguin)
(given (not guilty))    ; Negated fact
```

Facts produce **definite conclusions** (`+D`) that cannot be defeated.

## Strict Rules (`always`)

Strict rules express necessary implications. If the body is true, the head **must** be true.

```lisp
(always r1 penguin bird)              ; All penguins are birds
(always r2 (and human mortal) dies)   ; All mortal humans die
```

Strict rules produce **definite conclusions** (`+D`). They cannot be defeated by defeasible rules.

### When to Use Strict Rules

Use strict rules for:
- Definitional relationships (penguins are birds)
- Logical necessities (modus ponens)
- Constraints that have no exceptions

## Defeasible Rules (`normally`)

Defeasible rules express typical or default behavior that may have exceptions.

```lisp
(normally r1 bird flies)            ; Birds typically fly
(normally r2 student has-loans)     ; Students typically have loans
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

## Defeaters (`except`)

Defeaters are special rules that **block** conclusions without proving anything themselves.

```lisp
(except d1 broken-wing flies)    ; A broken wing blocks "flies"
```

### Defeater vs. Defeasible Rule

```lisp
; Defeasible rule: proves (not flies)
(normally r1 penguin (not flies))

; Defeater: only blocks flies, doesn't prove (not flies)
(except d1 sick flies)
```

The difference:
- `r1` can prove `(not flies)` if its body is satisfied
- `d1` can only block `flies`, it never proves `(not flies)`

### When to Use Defeaters

Use defeaters when:
- You want to express doubt without asserting the opposite
- Evidence should block a conclusion but not prove its negation
- You're modeling uncertainty

## Rule Bodies (Antecedents)

Rule bodies can contain:

### Single Literal
```lisp
(normally r1 bird flies)
```

### Multiple Literals (Conjunction)
```lisp
(normally r1 (and bird healthy) flies)
(normally r2 (and student employed) busy)
```

### Negated Literals
```lisp
(normally r1 (and bird (not penguin)) flies)   ; Non-penguin birds fly
```

## Rule Heads (Consequents)

Rule heads are single literals that can be:

### Positive
```lisp
(normally r1 bird flies)
```

### Negated
```lisp
(normally r1 penguin (not flies))
```

## Rule Labels

Every rule has a label (identifier) used for:
- Superiority relations
- Explanations
- Debugging

Labels are optional (auto-generated if omitted):
```lisp
(normally r1 bird flies)      ; labeled r1
(normally bird flies)         ; auto-labeled
```

## Summary

| Rule Type | SPL Keyword | Conclusion | Can be Defeated? |
|-----------|-------------|------------|------------------|
| Fact | `given` | +D | No |
| Strict | `always` | +D | No |
| Defeasible | `normally` | +d | Yes |
| Defeater | `except` | None (blocks only) | N/A |
