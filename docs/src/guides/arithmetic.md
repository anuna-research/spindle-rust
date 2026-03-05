# Arithmetic Expressions

Spindle supports arithmetic expressions in rule bodies for numeric computation, variable binding, and comparison guards.

## Overview

Arithmetic adds three capabilities to SPL rules:

1. **Expressions** — compute numeric values from operators and variables
2. **Bind constraints** — assign computed results to new variables
3. **Comparison guards** — filter substitutions based on numeric conditions

All arithmetic is restricted to rule bodies. Arithmetic cannot appear in facts, rule heads, or as standalone statements.

## Numeric Types

Spindle has three numeric types with automatic promotion:

| Type | Examples | Precision |
|------|----------|-----------|
| Integer | `42`, `-7`, `0` | Exact (64-bit signed) |
| Decimal | `3.14`, `0.001` | Exact (arbitrary precision) |
| Float | IEEE 754 double | Approximate |

### Promotion Rules

When mixing types in an operation, values are promoted along the chain:

```
Integer → Decimal → Float
```

- Integer + Integer = Integer
- Integer + Decimal = Decimal
- Anything + Float = Float

### Cross-Type Matching

During grounding, numeric values match across types when equal:

```lisp
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

```lisp
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

```lisp
(div 7 2)    ; => 3   (integer division, floor toward -inf)
(rem 7 2)    ; => 1   (remainder)
(** 2 10)    ; => 1024 (exponentiation)
```

`div` and `rem` require integer operands.

### Unary Operator

```lisp
(abs -5)            ; => 5
(abs (- 3 10))      ; => 7
```

### Rounding Functions

Three functions handle rounding:

```lisp
(round 2.55 1)      ; => 2.6  (1 decimal place, banker's rounding)
(round 2.45 1)      ; => 2.4  (rounds to even on half)
(round 3.14159 2)   ; => 3.14
(floor 3.7)         ; => 3
(floor -2.3)        ; => -3
(ceil 3.2)          ; => 4
(ceil -2.7)         ; => -2
```

`round` takes two arguments: the value and the number of decimal places (*dp*). It uses **banker's rounding** (half-to-even): when the value is exactly halfway, it rounds to the nearest even digit. This avoids systematic bias in large datasets.

`floor` and `ceil` each take one argument and return an Integer. `floor` returns the largest integer less than or equal to the value; `ceil` returns the smallest integer greater than or equal to it.

### Nesting

Expressions can be arbitrarily nested:

```lisp
(+ (* ?base ?rate) (abs (- ?adjustment ?threshold)))
```

## Bind Constraints

`bind` assigns the result of an expression to a variable:

```lisp
(bind ?total (+ ?price ?tax))
```

The variable must be unbound (not previously assigned in this rule). If it is already bound, the bind succeeds only if the existing value equals the computed result.

### Example: Computing Derived Values

```lisp
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

### Example: Rounding Computed Values

```lisp
(given (hours alice 37.5))
(given (rate alice 23.333))

(normally calc-pay
  (and (hours ?name ?h) (rate ?name ?r)
       (bind ?raw (* ?h ?r))
       (bind ?pay (round ?raw 2)))
  (weekly-pay ?name ?pay))
```

Result: `(weekly-pay alice 874.99)` — the raw product is rounded to 2 decimal places.

## Comparison Guards

Comparisons filter substitutions:

```lisp
(> ?age 18)
(<= ?score 100)
(= ?x ?y)
(!= ?status 0)
```

Available operators: `=`, `!=`, `<`, `>`, `<=`, `>=`

### Example: Filtering by Condition

```lisp
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

```lisp
(normally r1
  (and (budget ?b) (cost ?item ?c) (tax-rate ?r)
       (> ?b (+ ?c (* ?c ?r))))
  (affordable ?item))
```

## Evaluation Order

Body elements are evaluated **left to right**. Variables must be bound by a preceding literal or bind before they can be used in arithmetic:

```lisp
; CORRECT: ?price is bound before bind uses it
(normally r1
  (and (item ?name ?price)
       (bind ?discounted (* ?price 0.9)))
  (sale-price ?name ?discounted))
```

If an arithmetic expression references an unbound variable, the substitution is silently discarded (the rule does not fire for that ground instance).

## Arithmetic in Predicate Arguments

Arithmetic expressions can appear directly as predicate arguments in the body:

```lisp
(normally r1
  (and (base ?x ?b) (offset ?x ?o))
  (result ?x (+ ?b ?o)))
```

The expression `(+ ?b ?o)` is evaluated during grounding and the result becomes a concrete term in the head literal.

## Temporal Operator Overloads

The operators `+`, `-`, `*`, `/`, `min`, and `max` are also overloaded for temporal types (Date, Time, Datetime, Duration). For example, `Datetime + Duration` yields a Datetime, and `Date - Date` yields an Integer day count.

See the [Temporal Reasoning guide](temporal.md#temporal-arithmetic) for the full dispatch table, rules, and examples.

## Restrictions

### No Arithmetic in Heads or Facts (REQ-009)

```lisp
; INVALID — arithmetic in head
(normally r1 (price ?p) (bind ?total (* ?p 1.1)))

; INVALID — bind in a fact
(given (bind ?x 42))
```

### No Negated Arithmetic (REQ-011)

```lisp
; INVALID — cannot negate arithmetic constraints
(normally r1 (and (val ?x) (not (> ?x 100))) (low ?x))
```

Use the complementary comparison instead:

```lisp
; CORRECT
(normally r1 (and (val ?x) (<= ?x 100)) (low ?x))
```

### No Temporal Variables in Arithmetic (REQ-006)

Temporal variables (from `during` expressions) cannot be used as arithmetic operands.

### Reserved Keywords (REQ-008)

Arithmetic operators and comparison symbols cannot be used as predicate names or rule labels:

```
+  -  *  /  div  rem  abs  min  max  **
round  floor  ceil  bind  fold
=  !=  <  >  <=  >=
```

This also applies to tilde-negated forms (e.g., `~>` is rejected because `>` is reserved).

## Error Handling

| Error | Cause |
|-------|-------|
| Division by zero | `(/ ?x 0)` or `(div ?x 0)` |
| Non-integer operand | `(div 3.5 2)` or `(rem 1.5 1)` |
| Negative base with fractional exponent | `(** -2 0.5)` |
| Non-finite result | Overflow producing infinity or NaN |
| Unbound variable | Variable not yet assigned when expression is evaluated |
| Negative decimal places | `(round ?x -1)` — *dp* must be non-negative |
| Non-integer decimal places | `(round ?x 1.5)` — *dp* must be an integer |

When any of these occur during grounding, the substitution is discarded — the rule simply does not fire for that ground instance.
