# Rules and Facts

Spindle supports four types of rules, each with different semantics for how conclusions are drawn.

## Facts (`given`)

Facts are unconditional truths. They have no body (antecedent) and are always true.

```spl
(given bird)
(given penguin)
(given (not guilty))    ; Negated fact
```

Facts produce **definite conclusions** (`+D`) that cannot be defeated.

## Strict Rules (`always`)

Strict rules express necessary implications. If the body is true, the implication also makes the head true.

```spl
(always r1 penguin bird)              ; All penguins are birds
(always r2 (and human mortal) dies)   ; All mortal humans die
```

Strict rules produce **definite conclusions** (`+D`) when every premise is
definitely proved. With only defeasible support, a strict rule can instead
contribute `+d`, subject to conflict checks. Aggregate snapshot premises also
carry defeasible evidence. A definite conclusion cannot be defeated by a
defeasible rule.

### When to Use Strict Rules

Strict rules represent:
- Definitional relationships (penguins are birds)
- Logical necessities (modus ponens)
- Constraints that have no exceptions

## Defeasible Rules (`normally`)

Defeasible rules express typical or default behavior with possible exceptions.

```spl
(normally r1 bird flies)            ; Birds typically fly
(normally r2 student has-loans)     ; Students typically have loans
```

Defeasible rules produce **defeasible conclusions** (`+d`) that can be defeated by:
- Strict rules proving the opposite
- Superior defeasible rules
- Defeaters

### When to Use Defeasible Rules

Defeasible rules represent:
- Default behaviors with exceptions
- Typical properties
- Rules of thumb

## Defeaters (`except`)

Defeaters are special rules that attack the **complement of their head** without
proving the head. A defeater with head `(not flies)` blocks `flies`.
An applicable defeater can itself be overcome by superiority.

```spl
(except d1 broken-wing (not flies))    ; A broken wing blocks "flies"
```

### Defeater vs. Defeasible Rule

```spl
; Defeasible rule: proves (not flies)
(normally r1 penguin (not flies))

; Defeater: only blocks flies, doesn't prove (not flies)
(except d1 sick (not flies))
```

The difference:
- `r1` can prove `(not flies)` if its body is satisfied
- `d1` can only block `flies`, it never proves `(not flies)`

### When to Use Defeaters

Defeaters represent:
- Doubt without an assertion of the opposite
- Evidence that blocks a conclusion without proving its negation
- Uncertainty

## Rule Bodies (Antecedents)

Rule bodies can contain:

### Single Literal
```spl
(normally r1 bird flies)
```

### Multiple Literals (Conjunction)
```spl
(normally r1 (and bird healthy) flies)
(normally r2 (and student employed) busy)
```

### Negated Literals
```spl
(normally r1 (and bird (not penguin)) flies)   ; Non-penguin birds fly
```

## Rule Heads (Consequents)

Rule heads are single literals that can be:

### Positive
```spl
(normally r1 bird flies)
```

### Negated
```spl
(normally r1 penguin (not flies))
```

## Rule Labels

Every rule has a label (identifier) used for:
- Superiority relations
- Explanations
- Debugging

When labels are omitted, Spindle generates them automatically:
```spl
(normally r1 bird flies)      ; labeled r1
(normally bird flies)         ; auto-labeled
```

## Summary

| Rule Type | SPL Keyword | Conclusion | Can be Defeated? |
|-----------|-------------|------------|------------------|
| Fact | `given` | +D | No |
| Strict | `always` | +D from definite premises; otherwise potentially +d | Definite proof: no; defeasible support: yes |
| Defeasible | `normally` | +d | Yes |
| Defeater | `except` | None (blocks only) | N/A |
