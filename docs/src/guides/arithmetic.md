# Arithmetic Expressions

Spindle supports arithmetic expressions in rule bodies for numeric computation, variable binding, and comparison guards.

## Overview

Arithmetic adds three capabilities to SPL rules:

1. **Expressions** — compute numeric values from operators and variables
2. **Bind constraints** — assign computed results to new variables
3. **Comparison guards** — filter substitutions based on numeric conditions

SPL accepts arithmetic constraints and expression arguments in rule bodies.
It rejects arithmetic in facts, rule heads, and standalone statements.
Numeric values and variables bound by body expressions remain valid head arguments.

## Numeric Types

Spindle has three numeric types with automatic promotion:

| Type | Examples | Precision |
|------|----------|-----------|
| Integer | `42`, `-7`, `0` | Exact (64-bit signed) |
| Decimal | `3.14`, `0.001` | Exact (up to 28-29 significant digits) |
| Float | `1.5e2`, `1e-3` | Approximate (IEEE 754 double) |

### When Each Type Is Used

The parser chooses the numeric type based on how you write the literal:

- **Integer**: No decimal point, no exponent. `42`, `-7`, `0`.
- **Decimal**: Contains a decimal point but no exponent (`e`/`E`). `3.14`, `0.001`, `-0.5`.
- **Float**: Contains an exponent (`e` or `E`). `1.5e2` (= 150.0), `1e-3` (= 0.001), `2.0E10`.

Decimal is the default for numbers with a decimal point because it gives exact representation. This matters for financial calculations and precise comparisons: `0.1 + 0.2` equals exactly `0.3` in Decimal, but not in floating point. Scientific notation selects IEEE 754 semantics and supports very large or small magnitudes.

### Promotion Rules

When an operation mixes types, it promotes values along the chain:

```
Integer → Decimal → Float
```

- Integer + Integer = Integer
- Integer + Decimal = Decimal
- Anything + Float = Float

Once a Float enters a computation, the entire result is Float. This affects calculations that need exact precision.

### Cross-Type Matching

During grounding, numeric values match across types when equal:

```spl
(given (limit 100))          ; Integer
(given (score alice 100.0))  ; Decimal

; ?s (Decimal 100.0) matches ?limit (Integer 100) in the comparison
(normally r1
  (and (score ?name ?s) (limit ?limit) (>= ?s ?limit))
  (at-limit ?name))
```

## Operators

### N-ary Operators

These accept two or more arguments:

```spl
(+ 1 2 3)       ; => 6
(- 10 3 2)      ; => 5  (left fold: (10-3)-2)
(* 2 3 4)       ; => 24
(/ 100 5 2)     ; => 10 (left fold: (100/5)/2)
(min 5 3 8 1)   ; => 1
(max 5 3 8 1)   ; => 8
```

Subtraction and division use **left fold** semantics: `(- a b c)` = `((a - b) - c)`.

### Binary Operators

These require exactly two arguments:

```spl
(div 7 2)    ; => 3   (integer division, floor toward -inf)
(rem 7 2)    ; => 1   (remainder)
(** 2 10)    ; => 1024 (exponentiation)
```

`div` and `rem` require integer operands.

### Unary Operator

```spl
(abs -5)            ; => 5
(abs (- 3 10))      ; => 7
```

### Nesting

Expressions can be arbitrarily nested:

```spl
(+ (* ?base ?rate) (abs (- ?adjustment ?threshold)))
```

## Bind Constraints

`bind` assigns the result of an expression to a variable:

```spl
(bind ?total (+ ?price ?tax))
```

A new binding assigns an unbound variable (one without a previous assignment in this rule). If it is already bound, the bind succeeds only if the existing value equals the computed result.

### Example: Computing Derived Values

```spl
(given (item widget 25))
(given (item gadget 10))
(given (discount 0.15))

(normally calc-price
  (and (item ?name ?price) (discount ?rate)
       (bind ?savings (* ?price ?rate))
       (bind ?final (- ?price ?savings)))
  (final-price ?name ?final))
```

Results: `(final-price widget 21.25)`, `(final-price gadget 8.50)`

## Comparison Guards

Comparisons filter substitutions:

```spl
(> ?age 18)
(<= ?score 100)
(= ?x ?y)
(!= ?status 0)
```

Available operators: `=`, `!=`, `<`, `>`, `<=`, `>=`

### Example: Filtering by Condition

```spl
(given (employee alice 95000))
(given (employee bob 45000))
(given (employee carol 120000))

(normally high-earner
  (and (employee ?name ?salary) (> ?salary 90000))
  (senior-band ?name))
```

Only `alice` and `carol` satisfy `(> ?salary 90000)`.

### Comparisons with Expressions

Both sides can be expressions:

```spl
(normally r1
  (and (budget ?b) (cost ?item ?c) (tax-rate ?r)
       (> ?b (+ ?c (* ?c ?r))))
  (affordable ?item))
```

## Evaluation Order

The grounder evaluates body elements **left to right**. Arithmetic expressions need variables bound by a preceding literal or bind:

```spl
; CORRECT: ?price is bound before bind uses it
(normally r1
  (and (item ?name ?price)
       (bind ?discounted (* ?price 0.9)))
  (sale-price ?name ?discounted))
```

If an arithmetic expression references an unbound variable, the grounder silently discards the substitution. The rule does not fire for that ground instance.

## Arithmetic in Predicate Arguments

Arithmetic expressions can appear as predicate arguments in the body, but SPL rejects them in head arguments.
This head-expression example is invalid:

```spl
; INVALID — arithmetic expression in a head argument
(normally r1
  (and (base ?x ?b) (offset ?x ?o))
  (result ?x (+ ?b ?o)))
```

A body `bind` computes the result for a head variable:

```spl
(normally r1
  (and (base ?x ?b) (offset ?x ?o) (bind ?total (+ ?b ?o)))
  (result ?x ?total))
```

The grounder evaluates `(+ ?b ?o)` in the body. The bound `?total` becomes a concrete term in the head literal.

## Restrictions

Spindle enforces several restrictions on where arithmetic can appear. The parser checks each restriction and produces an error message.

### No Arithmetic in Heads or Facts (REQ-009)

Arithmetic constraints (`bind`, comparisons) filter and compute in rule bodies. They cannot appear as conclusions.
SPL also rejects expression arguments in heads and facts, as the preceding example illustrates.

```spl
; INVALID — bind in head position
(normally r1 (price ?p) (bind ?total (* ?p 1.1)))

; INVALID — bind as a fact
(given (bind ?x 42))

; INVALID — comparison in head position
(normally r1 bird (> 1 0))
```

**Error message:**

```
Arithmetic predicate 'bind' cannot appear in rule head or fact position (REQ-009)
```

The same message appears for comparison operators (`=`, `!=`, `<`, `>`, `<=`, `>=`) used as head literals.

### No Negated Arithmetic (REQ-011)

The `not` wrapper rejects arithmetic constraints. This avoids ambiguity about what "not greater than" means in a defeasible logic context.

```spl
; INVALID — cannot negate a comparison
(normally r1 (and (val ?x) (not (> ?x 100))) (low ?x))

; INVALID — cannot negate bind
(normally r1 (and bird (not (bind ?x 10))) flies)
```

**Error message:**

```
Arithmetic predicate '>' cannot be negated (REQ-011). Use the positive form in the rule body instead.
```

The complementary comparison expresses the opposite condition:

```spl
; CORRECT — use <= instead of (not >)
(normally r1 (and (val ?x) (<= ?x 100)) (low ?x))
```

### No Temporal Variables in Arithmetic (REQ-006)

Variables bound by `during` expressions represent time points or intervals, not numeric values. They cannot be used as arithmetic operands. If an arithmetic expression contains a temporal variable, the grounder silently discards the substitution. The rule does not fire for that ground instance.

```spl
; The rule below will never produce "shifted" because ?T is temporal
(given (during (event) 100 200))
(normally r1
  (and (during (event) ?T ?U) (bind ?next (+ ?T 1)))
  (shifted ?next))
```

### Reserved Keywords (REQ-008)

Arithmetic operators and comparison symbols cannot be used as predicate names or rule labels. This prevents confusing programs where `+` or `bind` look like user-defined predicates.

```
+  -  *  /  div  rem  abs  min  max  **
bind  =  !=  <  >  <=  >=
```

The following names are also reserved for future use: `sum`, `count`, `avg`, `round`, `floor`, `ceil`.

**Error message:**

```
Reserved keyword 'bind' cannot be used as a predicate name (REQ-008)
```

This also applies to tilde-negated forms (e.g., `~>` is rejected because `>` is reserved) and to rule labels in `prefer` declarations.

## Error Handling

### Runtime Errors (During Grounding)

| Error | Cause |
|-------|-------|
| Division by zero | `(/ ?x 0)` or `(div ?x 0)` |
| Non-integer operand | `(div 3.5 2)` or `(rem 1.5 1)` |
| Negative base with fractional exponent | `(** -2 0.5)` |
| Non-finite result | Overflow producing infinity or NaN |
| Unbound variable | Variable not yet assigned when expression is evaluated |
| Temporal variable in arithmetic | `(+ ?T 1)` where `?T` is from a `during` |

When any of these occur during grounding, the grounder discards the substitution. The rule does not fire for that ground instance. The grounder reports no error to the user. It silently skips the rule for that particular combination of variable bindings.

### Parse-Time Errors

| Error | Cause | Example |
|-------|-------|---------|
| Arithmetic in head (REQ-009) | `bind` or comparison used as a conclusion | `(normally r1 p (bind ?x 1))` |
| Negated arithmetic (REQ-011) | `not` wrapping `bind` or comparison | `(not (> ?x 5))` |
| Reserved keyword (REQ-008) | Operator used as predicate or label | `(given bind)` |
| Unknown operator | Unrecognised operator name | `(mod 5 3)` |
| Wrong arity | Too few or too many arguments | `(div 1)`, `(abs 1 2)` |
| Invalid operand | Non-numeric, non-variable atom | `(+ bird 1)` |

Parse-time errors halt processing and report the line number and a description of the problem.

## Rounding, value bindings, and aggregates

The builtin prelude includes `(round value decimal-places)` with half-to-even
rounding, `(floor value)`, and `(ceil value)`. Host applications can register
additional pure functions. `bind` can carry integer or symbol results from these
functions; builtin numeric guards still require numeric operands.

The `agg` form and explicit `fold` expressions in `bind` compute reductions across predicates.
The aggregate bridge currently uses checked integer arithmetic, even though
ordinary arithmetic supports decimals and floats. See
[Aggregation and Extension Functions](aggregation.md).
