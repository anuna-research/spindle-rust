# Negation

Spindle uses **strong negation** (also called explicit negation), not negation-as-failure.

## Strong Negation vs. Negation-as-Failure

### Negation-as-Failure (NAF)
Used in Prolog and many logic programming systems:
- "not P" means "P cannot be proven"
- Absence of evidence is evidence of absence

### Strong Negation
Used in Spindle:
- `-P` (or `~P`) is a separate proposition
- Must be explicitly proven
- Absence of `P` does not imply `-P`

## Syntax

Both `-` and `~` denote negation:

**DFL:**
```dfl
f1: >> -guilty       # Explicitly not guilty
r1: penguin => ~flies
```

**SPL:**
```lisp
(given (not guilty))
(normally r1 penguin (not flies))
```

## Three-Valued Outcomes

For any literal `P`, the outcome can be:

| State | Meaning | Conclusions |
|-------|---------|-------------|
| True | P is provable | `+d P` |
| False | -P is provable | `+d -P` |
| Unknown | Neither provable | `-d P` and `-d -P` |

## Example: Unknown State

```dfl
f1: >> bird
r1: bird => flies
# No information about grounded
```

Conclusions:
- `+d flies` (via r1)
- `-d grounded` (no rule proves it)
- `-d -grounded` (no rule proves its negation either)

The `grounded` literal is in an **unknown** state.

## Complementary Literals

`P` and `-P` are **complements**. They interact in specific ways:

### Conflict
If rules prove both `P` and `-P`:
```dfl
f1: >> a
r1: a => p
r2: a => -p
```
Without superiority, neither is provable (ambiguity).

### Definite Blocking
If `-P` is **definitely** provable, `P` cannot be defeasibly provable:
```dfl
f1: >> -p        # Definite fact: -p
r1: a => p       # Tries to prove p
```
Even if r1's body is satisfied, `+d p` is blocked because `+D -p`.

## Negation in Rule Bodies

Negated literals can appear in rule bodies:

```dfl
r1: bird, -penguin => flies    # Non-penguin birds fly
r2: student, -employed => needs_loan
```

**SPL:**
```lisp
(normally r1 (and bird (not penguin)) flies)
```

## Double Negation

`--P` is equivalent to `P`:

```dfl
r1: a => --flies    # Same as: a => flies
```

## Consistency

Spindle does not enforce consistency. A theory can prove both `P` and `-P`:

```dfl
f1: >> p
f2: >> -p
```

Result:
- `+D p`
- `+D -p`

This is an **inconsistent theory**. Spindle reports both conclusions without error.

### Detecting Inconsistency

To detect inconsistency, check for literals where both `+D P` and `+D -P` (or `+d P` and `+d -P`) are concluded.

## Closed World Assumption

Spindle does **not** use the Closed World Assumption by default. If you need it, add explicit rules:

```dfl
# CWA for 'guilty': if not proven guilty, then not guilty
d1: >> -guilty           # Default: not guilty
r1: evidence => guilty   # Can be overridden by evidence
r1 > d1
```

This pattern uses a defeatable default.
