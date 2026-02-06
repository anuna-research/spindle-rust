# Temporal Reasoning

Spindle supports timepoint ("as-of") reasoning, allowing you to reason about facts and rules active at a specific moment in time.

## Time Points

Time points are represented as milliseconds since the Unix epoch (UTC).

### Supported Formats

```lisp
(moment "2024-06-15T14:30:00Z")    ; RFC3339 / ISO 8601 string (UTC)
(moment 1718461800000)             ; Integer epoch milliseconds
inf                                ; Positive infinity (never ends)
-inf                               ; Negative infinity (always existed)
```

Note: Multi-arity forms (e.g., `(moment 2024 6 15)`) are not currently supported.

## During Operator

The `during` operator associates a literal with a time interval `[start, end]`.

Syntax: `(during literal start end)`

### Examples

```lisp
; Alice worked at Acme from 2020 to 2022
(given (during (employed alice acme) (moment "2020-01-01T00:00:00Z") (moment "2022-01-01T00:00:00Z")))

; Alice works at Beta from 2022 onwards
(given (during (employed alice beta) (moment "2022-01-01T00:00:00Z") inf))
```

## "As-Of" Reasoning

When reasoning with a reference time `t`, Spindle filters the theory:

1.  **Facts**: A fact `(during p start end)` is active if `start <= t <= end`.
2.  **Rules**: A rule is active if all its body literals and its head literal are active at `t`.

This allows querying the system state at any historical or future point.

## Limitations

1.  **Timepoint only**: Currently, Spindle supports reasoning *at* a timepoint. Interval inference (deriving new intervals from rules) is planned for a future release.
2.  **No Allen Relations**: Allen relations (`before`, `after`, `overlaps`, etc.) are not yet supported in the core reasoner.
3.  **No Interval Variables**: You cannot bind a variable to a time interval (e.g., `(during p ?t)` is not supported).
