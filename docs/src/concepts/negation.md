# Negation

Spindle uses **strong negation** (also called explicit negation), not negation-as-failure.

## Strong Negation vs. Negation-as-Failure

### Negation-as-Failure (NAF)
Used in Prolog and many logic programming systems:
- "not P" means "P cannot be proven"
- Absence of evidence is evidence of absence

### Strong Negation
Used in Spindle:
- `(not P)` is a separate proposition
- Must be explicitly proven
- Absence of `P` does not imply `(not P)`

## Syntax

```lisp
(given (not guilty))
(normally r1 penguin (not flies))
```

## Three-Valued Outcomes

For any literal `P`, the outcome can be:

| State | Meaning | Conclusions |
|-------|---------|-------------|
| True | P is provable | `+d P` |
| False | (not P) is provable | `+d -P` |
| Unknown | Neither provable | `-d P` and `-d -P` |

## Example: Unknown State

```lisp
(given bird)
(normally r1 bird flies)
; No information about grounded
```

Conclusions:
- `+d flies` (via r1)
- `-d grounded` (no rule proves it)
- `-d -grounded` (no rule proves its negation either)

The `grounded` literal is in an **unknown** state.

## Complementary Literals

`P` and `(not P)` are **complements**. They interact in specific ways:

### Conflict
If rules prove both `P` and `(not P)`:
```lisp
(given a)
(normally r1 a p)
(normally r2 a (not p))
```
Without superiority, neither is provable (ambiguity).

### Definite Blocking
If `(not P)` is **definitely** provable, `P` cannot be defeasibly provable:
```lisp
(given (not p))              ; Definite fact: (not p)
(normally r1 a p)            ; Tries to prove p
```
Even if r1's body is satisfied, `+d p` is blocked because `+D -p`.

## Negation in Rule Bodies

Negated literals can appear in rule bodies:

```lisp
(normally r1 (and bird (not penguin)) flies)          ; Non-penguin birds fly
(normally r2 (and student (not employed)) needs-loan)
```

## Consistency

Spindle does not enforce consistency. A theory can prove both `P` and `(not P)`:

```lisp
(given p)
(given (not p))
```

Result:
- `+D p`
- `+D -p`

This is an **inconsistent theory**. Spindle reports both conclusions without error.

### Detecting Inconsistency

To detect inconsistency, check for literals where both `+D P` and `+D -P` (or `+d P` and `+d -P`) are concluded.

## Closed World Assumption

Spindle does **not** use the Closed World Assumption by default. If you need it, add explicit rules:

```lisp
; CWA for 'guilty': if not proven guilty, then not guilty
(given (not guilty))                    ; Default: not guilty
(normally r1 evidence guilty)           ; Can be overridden by evidence
(prefer r1 f1)
```

This pattern uses a defeatable default.
