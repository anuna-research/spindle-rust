# SPL Format Reference

SPL (Spindle Lisp) is the input language for Spindle. It replaces the earlier DFL syntax with a LISP-based DSL.
Its s-expression structure supports new constructs without grammar ambiguity. These include temporal operators, trust directives, and claims blocks.

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
statement   = fact | rule | prefer | meta | predicate-decl
            | claims | trusts | decays | threshold

; Core
fact        = "(given" literal ")"
rule        = "(" keyword label? body head ")"
keyword     = "always" | "normally" | "except"
prefer      = "(prefer" label+ ")"
meta        = "(meta" meta-target property* ")"
meta-target = label | predicate-target

; Predicate declarations (SPEC-024)
predicate-decl   = "(predicate" functor argument-list ")"
argument-list    = "(" argument-decl* ")"
argument-decl    = "(" arg-name primitive-sort ")"
primitive-sort   = "symbol" | "integer" | "decimal" | "float" | "number" | "any"
predicate-target = "(predicate" functor arity ")"
arity            = "0" | non-zero-digit digit*

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
(except broken-wing-blocks-flight broken-wing (not flies))
```

### Unlabeled Rules

When labels are omitted, Spindle generates them automatically:

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

The `and` form combines multiple conditions:

```spl
(normally healthy-birds-fly (and bird healthy) flies)
(normally busy-students (and student employed) busy)
```

### Disjunctive conditions

Rule bodies do not support `or`. Separate rules with the same head express
disjunctive conditions. [Disjunctive Conditions](../concepts/disjunction.md)
explains the rationale and gives an example.

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

The wildcard `_` matches any value:

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

## Predicate Declarations

A predicate declaration records the *structure* of a predicate: its ordered argument names and primitive sorts.
This structure is independent of predicate usage. Declarations are not mandatory: undeclared predicates parse and reason exactly
as before. A declaration adds **no fact or rule**; it only populates the
theory's [vocabulary](#the-predicate-vocabulary).

```spl
(predicate assign-to
  ((task  symbol)
   (agent symbol)))

(predicate emergency ())        ; zero-arity predicate emergency/0
```

The number of argument declarations determines the arity, so
`assign-to/2` above has two positions and `emergency/0` has none.

### Inline Metadata

A declaration can carry trailing `meta` properties inline.
This syntax replaces a separate `(meta (predicate ...) ...)` statement in the common case:

```spl
(predicate assign-to
  ((task  symbol)
   (agent symbol))
  (description "Assign a task to an agent.")
  (tags ("planning" "scheduling")))
```

Inline properties are exact sugar for the [separate metadata
target](#predicate-metadata-targets). Both forms populate the same predicate metadata store.
The example equals a declaration plus a `(meta (predicate assign-to 2) ...)` statement with the same properties. Inline
and separate metadata for the same predicate merge (later values win per key).

### Primitive Sorts

Each argument declares one primitive sort. Sorts describe the *value space*
only; they carry no domain meaning and do not affect inference.

| Sort | Accepts |
|---|---|
| `symbol` | an interned symbolic name |
| `integer` | a 64-bit integer |
| `decimal` | an fixed-precision decimal |
| `float` | a finite IEEE-754 float |
| `number` | any of `integer`, `decimal`, or `float` |
| `any` | any ground term |

The parser checks declarations. Argument names need non-empty, unique values; an unknown sort causes a parse error.

### Predicate Indicators

For display and CLI interoperability, predicates use Prolog-style `functor/arity` notation, such as `assign-to/2` or `emergency/0`. When a functor contains `/`, its display includes quotes: `"rate/limit"/2`. This notation is presentation only —
the machine representation always keeps functor and arity as separate
structured fields, never a parsed string.

## Metadata

Metadata attaches to a rule by label or to a predicate by structured target:

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

### Predicate Metadata Targets

A `meta` target can be a structured `(predicate functor arity)` selector
instead of a label. This attaches metadata to one predicate symbol without
overloading a rule label or parsing a predicate-indicator string:

```spl
(predicate assign-to ((task symbol) (agent symbol)))

(meta (predicate assign-to 2)
  (description "Assign a task to an agent."))
```

The metadata store distinguishes predicate targets from labels and other arities.
Metadata for `(predicate assign-to 2)` never collides with a rule labelled `assign-to`, nor with `assign-to/1`. Predicate descriptions do not
have to be declared — they can annotate any predicate the theory uses.

### The Predicate Vocabulary

Declarations, predicate metadata, and observed rule usage together form a derived, read-only **vocabulary**.
This catalogue uses predicate symbols as keys.
Each entry carries a signature, a description, observed argument kinds per position, and the rule occurrences that reference the predicate. The vocabulary is a
tooling projection — it never changes conclusions. See the
[Rust Library guide](../integration/rust.md#predicate-vocabulary) for the API
(`TheorySignature`, `Vocabulary`).

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

SPL supports interval variables such as `(during p ?T)` and all 13 Allen
constraints in rule bodies. The `within` keyword names Allen's During relation, distinct
from the [`during` SPL wrapper](#during). For example:

```spl
(given (during p 1 10))
(given (during q 20 30))
(normally sequence
  (and (during p ?T) (during q ?S) (before ?T ?S))
  ordered)
```

See [Temporal Reasoning](../guides/temporal.md) for interval propagation,
state constraints, family matching, and as-of filtering.

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
3.14        ; Decimal (fixed precision)
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
| `round` | Binary | Half-to-even rounding to decimal places |
| `floor` | Unary | Round down to an integer |
| `ceil` | Unary | Round up to an integer |

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

A bind assigns an arithmetic expression’s result to a variable:

```spl
(normally compute-total
  (and (price ?p) (tax-rate ?r)
       (bind ?total (+ ?p (* ?p ?r))))
  (total ?total))
```

### Comparison Guards

Comparison guards compare two arithmetic expressions:

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

Arithmetic promotes numeric types: Integer → Decimal → Float.

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

The `claims` block attributes statements to a named source, with metadata when supplied.

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
- **:at** — RFC3339 timestamp for when the claim was made.
- **:sig** — signature string for verification.
- **:id** — block identifier.
- **:note** — free-text annotation.

Statements inside a `claims` block are ordinary SPL expressions (`given`, `always`, `normally`, `except`, `prefer`) that automatically receive source metadata. See the [Trust & Multi-Agent guide](../guides/trust.md) for details.

## Complete Example

```spl
; The Penguin Example

; Predicate declaration + metadata (optional structural documentation)
(predicate flies ())
(meta (predicate flies 0) (description "Capable of flight."))

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
(except broken-wing-blocks-flight broken-wing (not flies))

; Metadata
(meta birds-fly (description "Birds typically fly"))
(meta penguins-dont-fly (description "Penguins are an exception"))
```


## Aggregation and extension calls

```spl
(given (payment alice first 10))
(given (payment alice second 10))
(normally total
  (agg ?total sum ?amount (payment ?person ?id ?amount))
  (total-payment ?total))
```

This derives `(total-payment 20)`. Named aggregators are `sum`, `count`, `min-of`,
and `max-of`. The output and contribution are variables; the contribution needs to
occur in the single row pattern. For grouped results, earlier ordinary premises bind grouping variables. Separate helper predicates can express joins/filters.

Explicit folds remain available:

```spl
(normally total
  (bind ?total (fold + ?amount :from (payment ?person ?id ?amount) :initial 0))
  (total-payment ?total))
```

A fold appears directly in `bind` and has one `:from` pattern and either
`:initial expression` or `:require-nonempty`. Nested folds require separate
rules/strata. Aggregation is a rule premise, never a fact, head, or negated premise.
The current bridge supports checked integer/symbol values; decimal/float, modal,
temporal, and trust-weighted aggregate programs are unsupported.

`bind` also accepts registered extension calls. Unknown functions and invalid
arities are preparation errors; parsing does not load host code. See
[Aggregation and Extension Functions](../guides/aggregation.md) for scoping,
empty inputs, snapshot evidence, custom registries, and limits.
