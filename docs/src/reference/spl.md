# SPL Format Reference

SPL (Spindle Lisp) is a LISP-based DSL for defeasible logic with support for advanced features like variables, temporal reasoning, and metadata.

## File Extension

`.spl`

## Comments

Semicolon to end of line:

```lisp
; This is a comment
(given bird)  ; Inline comment
```

## Grammar Overview

```ebnf
theory      = statement*
statement   = fact | rule | prefer | meta
fact        = "(given" literal ")"
rule        = "(" keyword label? body head ")"
keyword     = "always" | "normally" | "except"
prefer      = "(prefer" label+ ")"
meta        = "(meta" label property* ")"
literal     = atom | "(" atom arg* ")" | "(not" literal ")"
body        = literal | "(and" literal+ ")"
atom        = identifier | variable
variable    = "?" identifier
```

## Facts

### Simple Facts

```lisp
(given bird)
(given penguin)
(given (not guilty))
```

### Predicate Facts

```lisp
(given (parent alice bob))
(given (employed alice acme))
```

### Flat Predicate Syntax

```lisp
(given parent alice bob)     ; Same as (given (parent alice bob))
(given employed alice acme)
```

## Rules

### Strict Rules (`always`)

```lisp
(always r1 penguin bird)
(always r2 (and human mortal) dies)
```

### Defeasible Rules (`normally`)

```lisp
(normally r1 bird flies)
(normally r2 penguin (not flies))
```

### Defeaters (`except`)

```lisp
(except d1 broken-wing flies)
```

### Unlabeled Rules

Labels are optional (auto-generated):

```lisp
(normally bird flies)        ; Gets label like "r1"
(always penguin bird)        ; Gets label like "s1"
```

## Literals

### Simple

```lisp
bird
flies
has-feathers
```

### Negated

```lisp
(not flies)
(not (parent alice bob))
```

Or with prefix:
```lisp
~flies
```

### Predicates with Arguments

```lisp
(parent alice bob)
(employed ?x acme)
(ancestor ?x ?z)
```

## Conjunction

Use `and` for multiple conditions:

```lisp
(normally r1 (and bird healthy) flies)
(normally r2 (and student employed) busy)
```

## Variables

Variables start with `?`:

```lisp
(given (parent alice bob))
(given (parent bob charlie))

; Transitive closure
(normally r1 (parent ?x ?y) (ancestor ?x ?y))
(normally r2 (and (parent ?x ?y) (ancestor ?y ?z)) (ancestor ?x ?z))
```

### Wildcard

Use `_` to match anything:

```lisp
(normally r1 (parent _ ?y) (has-parent ?y))
```

## Superiority

### Two Rules

```lisp
(prefer r2 r1)    ; r2 > r1
```

### Chain

```lisp
(prefer r3 r2 r1)  ; r3 > r2 > r1
```

Expands to:
```lisp
(prefer r3 r2)
(prefer r2 r1)
```

## Metadata

Attach metadata to rules:

```lisp
(meta r1
  (description "Birds normally fly")
  (confidence 0.9)
  (source "ornithology-handbook"))
```

### Properties

```lisp
(meta rule-label
  (key "string value")
  (key2 ("list" "of" "values")))
```

## Modal Operators

### Obligation (`must`)

```lisp
(normally signed-contract (must pay))
```

### Permission (`may`)

```lisp
(normally member (may access))
```

### Forbidden (`forbidden`)

```lisp
(normally unauthorized (forbidden enter))
```

## Temporal Reasoning

### Time Points

Supported time formats:

```lisp
(moment "2024-06-15T14:30:00Z")  ; RFC3339 / ISO 8601
1718461800000                    ; Epoch milliseconds
inf                              ; Positive infinity
-inf                             ; Negative infinity
```

> Note: Multi-arity forms like `(moment 2024 6 15)` are reserved for future extensions.

### During

```lisp
(given (during bird 1 10))
(given (during (employed alice acme)
  (moment "2020-01-01T00:00:00Z")
  (moment "2023-01-01T00:00:00Z")))
```

### Allen Relations

*Not currently supported in Spindle v1.*

## Complete Example

```lisp
; The Penguin Example

; Facts
(given bird)
(given penguin)

; Strict rule
(always s1 penguin bird)

; Defeasible rules
(normally r1 bird flies)
(normally r2 bird has-feathers)
(normally r3 penguin (not flies))
(normally r4 penguin swims)

; Superiority
(prefer r3 r1)

; Defeater
(except d1 broken-wing flies)

; Metadata
(meta r1 (description "Birds typically fly"))
(meta r3 (description "Penguins are an exception"))
```

