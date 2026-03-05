# SPL Format Reference

SPL (Spindle Lisp) is the input language for Spindle. It replaces the earlier DFL syntax with a LISP-based DSL whose s-expression structure makes it straightforward to add new constructs (temporal operators, trust directives, claims blocks, etc.) without grammar ambiguity.

## File Extension

`.spl`

## Comments

Semicolon to end of line:

```spl
; This is a comment
(given bird)  ; Inline comment
```

## Grammar Overview

```ebnf
theory      = statement*
statement   = fact | rule | prefer | meta
            | claims | trusts | decays | threshold

; Core
fact        = "(given" literal ")"
rule        = "(" keyword label? body head ")"
keyword     = "always" | "normally" | "except"
prefer      = "(prefer" label+ ")"
meta        = "(meta" label property* ")"

; Trust
claims      = "(claims" source claims-meta* statement* ")"
claims-meta = ":at" atom | ":sig" atom | ":id" atom | ":note" atom
trusts      = "(trusts" source number ")"
decays      = "(decays" source decay-model number ")"
decay-model = "exponential" | "linear" | "step"
threshold   = "(threshold" name number ")"

; Temporal
time-expr   = "(moment" rfc3339-string ")" | integer | "inf" | "-inf"
during      = "(during" literal time-expr time-expr ")"

; Literals
literal     = atom | "(" atom arg* ")" | "(not" literal ")"
            | during | modal
modal       = "(" modal-op literal ")"
modal-op    = "must" | "may" | "forbidden"
body        = literal | "(and" body-elem+ ")"
body-elem   = literal | arith-constraint
atom        = identifier | variable
variable    = "?" identifier
source      = atom
number      = float in [0.0, 1.0]

; Arithmetic (body only)
arith-constraint = bind | compare
bind        = "(bind" variable arith-expr ")"
compare     = "(" cmp-op arith-expr arith-expr ")"
cmp-op      = "=" | "!=" | "<" | ">" | "<=" | ">="
arith-expr  = number | variable
            | "(" nary-op arith-expr+ ")"
            | "(" bin-op arith-expr arith-expr ")"
            | "(" unary-op arith-expr ")"
nary-op     = "+" | "-" | "*" | "/" | "min" | "max"
bin-op      = "div" | "rem" | "**"
unary-op    = "abs"
```

## Facts

### Simple Facts

```spl
(given bird)
(given penguin)
(given (not guilty))
```

### Predicate Facts

```spl
(given (parent alice bob))
(given (employed alice acme))
```

### Flat Predicate Syntax

```spl
(given parent alice bob)     ; Same as (given (parent alice bob))
(given employed alice acme)
```

## Rules

### Strict Rules (`always`)

```spl
(always penguins-are-birds penguin bird)
(always mortals-die (and human mortal) dies)
```

### Defeasible Rules (`normally`)

```spl
(normally birds-fly bird flies)
(normally penguins-dont-fly penguin (not flies))
```

### Defeaters (`except`)

```spl
(except broken-wing-blocks-flight broken-wing flies)
```

### Unlabeled Rules

Labels are optional (auto-generated):

```spl
(normally bird flies)        ; Gets label like "r1"
(always penguin bird)        ; Gets label like "s1"
```

## Literals

### Simple

```spl
bird
flies
has-feathers
```

### Negated

```spl
(not flies)
(not (parent alice bob))
```

Or with prefix:
```spl
~flies
```

### Predicates with Arguments

```spl
(parent alice bob)
(employed ?x acme)
(ancestor ?x ?z)
```

## Conjunction

Use `and` for multiple conditions:

```spl
(normally healthy-birds-fly (and bird healthy) flies)
(normally busy-students (and student employed) busy)
```

### Why there is no `or`

Defeasible logic does not support disjunction in rule bodies. This is by design, not a missing feature — the proof theory (Nute/Billington/Antoniou) defines derivation over conjunctive rules only. Adding `or` would break the well-defined defeat and superiority semantics that make defeasible reasoning tractable.

Instead, express disjunction as separate rules that conclude the same head:

```spl
; "If rain or snow, take umbrella"
(normally rain-means-umbrella rain take-umbrella)
(normally snow-means-umbrella snow take-umbrella)
```

Each rule independently supports the same conclusion. If either `rain` or `snow` is provable, `take-umbrella` follows. This is the standard encoding of disjunctive conditions in defeasible logic, and it preserves per-rule defeat — you can override one path without affecting the other.

## Variables

Variables start with `?`:

```spl
(given (parent alice bob))
(given (parent bob charlie))

; Transitive closure
(normally parent-is-ancestor (parent ?x ?y) (ancestor ?x ?y))
(normally ancestor-chain (and (parent ?x ?y) (ancestor ?y ?z)) (ancestor ?x ?z))
```

### Wildcard

Use `_` to match anything:

```spl
(normally has-any-parent (parent _ ?y) (has-parent ?y))
```

## Superiority

### Two Rules

```spl
(prefer penguins-dont-fly birds-fly)
```

### Chain

```spl
(prefer emergency-override safety-protocol standard-rule)
```

Expands to:
```spl
(prefer emergency-override safety-protocol)
(prefer safety-protocol standard-rule)
```

## Metadata

Attach metadata to rules:

```spl
(meta birds-fly
  (description "Birds normally fly")
  (confidence 0.9)
  (source "ornithology-handbook"))
```

### Properties

```spl
(meta rule-label
  (key "string value")
  (key2 ("list" "of" "values")))
```

## Modal Operators

### Obligation (`must`)

```spl
(normally contract-requires-payment signed-contract (must pay))
```

### Permission (`may`)

```spl
(normally members-may-access member (may access))
```

### Forbidden (`forbidden`)

```spl
(normally no-entry-unauthorized unauthorized (forbidden enter))
```

## Temporal Reasoning

### Time Points

Supported time formats:

```spl
(moment "2024-06-15T14:30:00Z")  ; RFC3339 / ISO 8601
1718461800000                    ; Epoch milliseconds
inf                              ; Positive infinity
-inf                             ; Negative infinity
```

> Note: Multi-arity forms like `(moment 2024 6 15)` are reserved for future extensions.

### During

```spl
(given (during bird 1 10))
(given (during (employed alice acme)
  (moment "2020-01-01T00:00:00Z")
  (moment "2023-01-01T00:00:00Z")))
```

### Allen Relations

> **SPL status:** Allen relations are implemented in the core Rust API but are not yet
> usable as SPL predicates. Exposing them requires interval variables (e.g.,
> `(during p ?t)`), planned for a future release. The Allen `during` relation
> (interval containment) is distinct from the [`during` SPL operator](#during)
> described above.

Allen's interval algebra defines exactly 13 mutually exclusive relations between
two time intervals **X** and **Y**. Every pair of intervals satisfies exactly one.

```text
Relation          X              Y           Inverse
─────────────────────────────────────────────────────────
before            ██████                     after
                              ██████

meets             ██████                     met-by
                        ██████

overlaps          ██████                     overlapped-by
                     ██████

starts            ████                       started-by
                  ██████████

during              ████                     contains
                  ██████████

finishes              ████                   finished-by
                  ██████████

equals            ██████████
                  ██████████                 (self-inverse)
```

Each relation has a strict inverse (reading the diagram with X and Y swapped),
giving 6 symmetric pairs plus `equals`:

| Relation | Inverse | Condition |
|---|---|---|
| `before` | `after` | X ends before Y starts (with gap) |
| `meets` | `met-by` | X ends exactly where Y starts |
| `overlaps` | `overlapped-by` | X starts first, they share some time, Y ends last |
| `starts` | `started-by` | Both start together, X ends first |
| `during` | `contains` | X is fully enclosed within Y |
| `finishes` | `finished-by` | Both end together, X starts later |
| `equals` | `equals` | Identical start and end |

## Arithmetic Expressions

Arithmetic expressions can appear in rule bodies as `bind` constraints, comparison guards, or as arguments to predicates.

### Numeric Literals

```spl
42          ; Integer
3.14        ; Decimal (arbitrary precision)
```

### Operators

| Operator | Arity | Description |
|----------|-------|-------------|
| `+` | N-ary | Addition |
| `-` | N-ary | Subtraction (left fold) |
| `*` | N-ary | Multiplication |
| `/` | N-ary | Division (left fold) |
| `div` | Binary | Integer division (floor) |
| `rem` | Binary | Remainder |
| `**` | Binary | Exponentiation |
| `abs` | Unary | Absolute value |
| `min` | N-ary | Minimum |
| `max` | N-ary | Maximum |

```spl
(+ 1 2)           ; => 3
(* 2 3 4)         ; => 24
(- 10 3 2)        ; => 5 (left fold: 10-3-2)
(div 7 2)         ; => 3
(rem 7 2)         ; => 1
(** 2 10)         ; => 1024
(abs (- 3 10))    ; => 7
(min 5 3 8)       ; => 3
```

### Bind Constraints

Bind a variable to the result of an arithmetic expression:

```spl
(normally compute-total
  (and (price ?p) (tax-rate ?r)
       (bind ?total (+ ?p (* ?p ?r))))
  (total ?total))
```

### Comparison Guards

Compare two arithmetic expressions:

```spl
(normally adult-by-age
  (and (age ?x ?a) (> ?a 18))
  (adult ?x))

(normally failing-score
  (and (score ?x ?s) (<= ?s 50))
  (failing ?x))
```

Available operators: `=`, `!=`, `<`, `>`, `<=`, `>=`

### Arithmetic in Predicate Arguments

Arithmetic expressions can appear as predicate arguments in rule bodies:

```spl
(normally compute-invoice
  (and (price ?item ?p) (tax-rate ?r))
  (invoice ?item (+ ?p (* ?p ?r))))
```

### Type Promotion

Numeric types are promoted during arithmetic: Integer → Decimal → Float.

- Integer + Integer = Integer
- Integer + Decimal = Decimal
- Any + Float = Float
- `div` and `rem` require integer operands

Cross-type matching: `Integer(2)` matches `Decimal(2.0)` matches `Float(2.0)`.

### Reserved Keywords (REQ-008)

The following cannot be used as predicate names or rule labels:

```
+  -  *  /  div  rem  abs  min  max  **
bind  =  !=  <  >  <=  >=
```

Future reserved: `sum`, `count`, `avg`, `round`, `floor`, `ceil`

### Restrictions

- Arithmetic constraints cannot appear in rule heads or facts (REQ-009)
- Arithmetic constraints cannot be negated with `not` or `~` (REQ-011)
- Temporal variables cannot be used as arithmetic operands (REQ-006)

## Claims

The `claims` block attributes statements to a named source, with optional metadata.

```spl
(claims agent:alice
  :at "2024-06-15T12:00:00Z"
  :sig "abc123"
  :id "claim-001"
  :note "sensor reading"
  (given sunny)
  (normally no-umbrella-when-sunny sunny (not umbrella)))
```

### Syntax

```
(claims source [:at timestamp] [:sig signature] [:id block-id] [:note annotation]
  statement ...)
```

- **source** — an atom identifying the claiming agent (e.g., `agent:alice`).
- **:at** — optional RFC3339 timestamp for when the claim was made.
- **:sig** — optional signature string for verification.
- **:id** — optional block identifier.
- **:note** — optional free-text annotation.

Statements inside a `claims` block are ordinary SPL expressions (`given`, `always`, `normally`, `except`, `prefer`) that automatically receive source metadata. See the [Trust & Multi-Agent guide](../guides/trust.md) for details.

## Complete Example

```spl
; The Penguin Example

; Facts
(given bird)
(given penguin)

; Strict rule
(always penguins-are-birds penguin bird)

; Defeasible rules
(normally birds-fly bird flies)
(normally birds-have-feathers bird has-feathers)
(normally penguins-dont-fly penguin (not flies))
(normally penguins-swim penguin swims)

; Superiority — specificity
(prefer penguins-dont-fly birds-fly)

; Defeater
(except broken-wing-blocks-flight broken-wing flies)

; Metadata
(meta birds-fly (description "Birds typically fly"))
(meta penguins-dont-fly (description "Penguins are an exception"))
```

