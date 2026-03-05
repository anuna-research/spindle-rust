# SPL Format Reference

SPL (Spindle Lisp) is the input language for Spindle. It replaces the earlier DFL syntax with a LISP-based DSL whose s-expression structure makes it straightforward to add new constructs (temporal operators, trust directives, claims blocks, etc.) without grammar ambiguity.

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
body-elem   = literal | arith-constraint | fold
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
unary-op    = "abs" | "floor" | "ceil"
round-op    = "round"

; Aggregation (body only)
fold        = "(fold" variable identity reducer extract pattern ")"
identity    = arith-expr | "required"
reducer     = atom
extract     = arith-expr
pattern     = literal
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

## Date/Time Types

SPL provides five temporal value types for calendar and clock arithmetic. Temporal literals use a `#` prefix to distinguish them from symbols.

### Literal Syntax

| Type | Syntax | Example | Internal |
|------|--------|---------|----------|
| Date | `#d:YYYY-MM-DD` | `#d:2025-07-15` | Calendar date |
| Time | `#t:HH:MM` | `#t:09:00` | Minutes since midnight (0–1439) |
| Datetime | `#dt:YYYY-MM-DDTHH:MM±HH:MM` | `#dt:2025-07-15T14:00+10:00` | Timezone-aware instant |
| Duration | `#dur:<components>` | `#dur:1d6h30m` | Signed minutes |
| Offset | `#off:±HH:MM` or `#off:Z` | `#off:+10:00` | Signed minutes east of UTC |

Datetime also accepts `Z` for UTC: `#dt:2025-07-15T14:00Z`.

Duration components are `d` (days = 1440 min), `h` (hours = 60 min), `m` (minutes). At least one component is required. Equal total minutes are equal: `#dur:1d12h` = `#dur:36h`.

### Grammar Fragment

```ebnf
temporal    = date | time | datetime | duration | offset
date        = "#d:" YYYY "-" MM "-" DD
time        = "#t:" HH ":" MM
datetime    = "#dt:" YYYY "-" MM "-" DD "T" HH ":" MM offset-suffix
offset-suffix = "Z" | ("+" | "-") HH ":" MM
duration    = "#dur:" ["-"] component+
component   = digits ("d" | "h" | "m")
offset      = "#off:" ("Z" | ("+" | "-") HH ":" MM)
```

### Equality and Ordering

- **Date, Time, Duration, Offset**: Structural equality and natural ordering.
- **Datetime**: Equality and ordering compare by **instant** (UTC equivalent). Two Datetimes representing the same moment in different offsets are equal.

## Temporal Functions

Temporal functions are available in `bind` expressions within rule bodies.

### Construction

| Function | Signature | Description |
|----------|-----------|-------------|
| `datetime` | `(Date, Time, Offset) → Datetime` | Compose a Datetime from parts |

```lisp
(bind ?dt (datetime #d:2025-07-15 #t:14:00 #off:+10:00))  ; => #dt:2025-07-15T14:00+10:00
```

### Extraction

| Function | Signature | Description |
|----------|-----------|-------------|
| `date-of` | `(Datetime) → Date` | Extract local date |
| `time-of` | `(Datetime) → Time` | Extract local time |
| `day-of-week` | `(Date \| Datetime) → Symbol` | `:monday` .. `:sunday` |
| `year-of` | `(Date \| Datetime) → Integer` | Year component |
| `month-of` | `(Date \| Datetime) → Integer` | Month (1–12) |
| `day-of-month` | `(Date \| Datetime) → Integer` | Day (1–31) |

```lisp
(bind ?d (date-of ?dt))                  ; => #d:2025-07-15
(bind ?dow (day-of-week #d:2025-07-14))  ; => :monday
(bind ?y (year-of #d:2025-12-25))        ; => 2025
```

### Difference

| Function | Signature | Description |
|----------|-----------|-------------|
| `hours-between` | `(Datetime, Datetime) → Decimal` | Signed hours (dt2 − dt1) |
| `minutes-between` | `(Datetime, Datetime) → Integer` | Signed minutes (dt2 − dt1) |
| `days-between` | `(Date, Date) → Integer` | Signed days (date2 − date1) |
| `duration-hours` | `(Duration) → Decimal` | Total hours |
| `duration-minutes` | `(Duration) → Integer` | Total minutes |

```lisp
(bind ?d (days-between #d:2025-07-01 #d:2025-07-15))  ; => 14
(bind ?h (duration-hours #dur:1h30m))                   ; => 1.5
```

### Calendar Arithmetic

| Function | Signature | Description |
|----------|-----------|-------------|
| `add-months` | `(Date, Integer) → Date` | Add calendar months (day clamped) |
| `add-years` | `(Date, Integer) → Date` | Add calendar years (day clamped) |
| `months-between` | `(Date, Date) → Integer` | Complete calendar months |
| `years-between` | `(Date, Date) → Integer` | Complete calendar years |

Day clamping: when the target month has fewer days, the day is clamped to the last valid day. `add-months(#d:2025-01-31, 1)` → `#d:2025-02-28`.

```lisp
(bind ?review (add-months #d:2025-01-31 1))              ; => #d:2025-02-28
(bind ?y (years-between #d:2023-03-01 #d:2025-03-01))   ; => 2
```

### Temporal Operator Overloads

The arithmetic operators `+`, `-`, `*`, `/`, `min`, and `max` are overloaded for temporal types. See the [Temporal Reasoning guide](../guides/temporal.md#temporal-arithmetic) for the full dispatch table and examples.

## Arithmetic Expressions

Arithmetic expressions can appear in rule bodies as `bind` constraints, comparison guards, or as arguments to predicates.

### Numeric Literals

```lisp
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
| `round` | Binary | Banker's rounding (half-to-even) to *dp* decimal places |
| `floor` | Unary | Floor to integer |
| `ceil` | Unary | Ceiling to integer |

```lisp
(+ 1 2)           ; => 3
(* 2 3 4)         ; => 24
(- 10 3 2)        ; => 5 (left fold: 10-3-2)
(div 7 2)         ; => 3
(rem 7 2)         ; => 1
(** 2 10)         ; => 1024
(abs (- 3 10))    ; => 7
(min 5 3 8)       ; => 3
(round 2.55 1)    ; => 2.6 (banker's rounding)
(floor 3.7)       ; => 3
(ceil 3.2)        ; => 4
```

### Bind Constraints

Bind a variable to the result of an arithmetic expression:

```lisp
(normally r1
  (and (price ?p) (tax-rate ?r)
       (bind ?total (+ ?p (* ?p ?r))))
  (total ?total))
```

### Comparison Guards

Compare two arithmetic expressions:

```lisp
(normally r1
  (and (age ?x ?a) (> ?a 18))
  (adult ?x))

(normally r2
  (and (score ?x ?s) (<= ?s 50))
  (failing ?x))
```

Available operators: `=`, `!=`, `<`, `>`, `<=`, `>=`

### Arithmetic in Predicate Arguments

Arithmetic expressions can appear as predicate arguments in rule bodies:

```lisp
(normally r1
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
round  floor  ceil  bind  fold
=  !=  <  >  <=  >=
```

Future reserved: `sum`, `count`, `avg`

### Restrictions

- Arithmetic constraints cannot appear in rule heads or facts (REQ-009)
- Arithmetic constraints cannot be negated with `not` or `~` (REQ-011)
- Temporal variables cannot be used as arithmetic operands (REQ-006)

## Fold Aggregation

The `fold` construct aggregates values across all facts matching a pattern. It appears in rule bodies alongside other literals and constraints.

### Syntax

```lisp
(fold ?result identity reducer extract pattern)
```

| Component | Description |
|-----------|-------------|
| `?result` | Variable that receives the aggregated value |
| `identity` | Starting value (an arithmetic expression) or `required` |
| `reducer` | Binary function applied to combine values (e.g., `+`, `min`, `max`) |
| `extract` | Arithmetic expression evaluated per matching fact |
| `pattern` | A literal to match against (must be positive — no `not`) |

### Examples

**Sum of values:**

```lisp
(normally r-total
    (fold ?total 0 + ?pay (pay-line ?emp ?pay))
    (total-pay ?emp ?total))
```

For each `?emp`, sums all `?pay` values from matching `pay-line` facts. The identity `0` means the rule still fires (with result `0`) if no facts match.

**Count:**

```lisp
(normally r-count
    (fold ?n 0 + 1 (shift ?emp ?d))
    (shift-count ?emp ?n))
```

The extract `1` contributes a constant per match; summing counts them.

**Minimum with required:**

```lisp
(normally r-min
    (fold ?m required min ?rate (rate ?emp ?rate))
    (min-rate ?emp ?m))
```

The keyword `required` means the rule does not fire when no facts match (there is no sensible minimum of an empty set).

**Computed extract:**

```lisp
(normally r-total
    (fold ?total 0 + (* ?hours ?rate) (work ?emp ?hours ?rate))
    (daily-pay ?emp ?total))
```

The extract `(* ?hours ?rate)` is evaluated per match before folding.

### Identity vs. Required

- **Identity value** (e.g., `0`, `1`): The fold returns this value when no facts match the pattern. The rule fires normally.
- **`required`**: The fold fails when no facts match. The substitution is discarded and the rule does not fire for that binding.

Use `required` for aggregates where an empty set is meaningless (min, max). Use an identity for aggregates where zero matches should produce a result (sum → `0`, count → `0`, product → `1`).

### Restrictions

- Fold can only appear in rule bodies, not in heads or facts
- The pattern must be positive (no `not` or `~`)
- The result variable must not collide with any variable bound in the fold pattern
- The reducer must be a binary function (built-in operator or registered extension function)
- Temporal features cannot be combined with multi-stratum fold programs

See the [Aggregation guide](../guides/aggregation.md) for variable scoping, stratification, and advanced patterns.

## Claims

The `claims` block attributes statements to a named source, with optional metadata.

```lisp
(claims agent:alice
  :at "2024-06-15T12:00:00Z"
  :sig "abc123"
  :id "claim-001"
  :note "sensor reading"
  (given sunny)
  (normally r1 sunny (not umbrella)))
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

