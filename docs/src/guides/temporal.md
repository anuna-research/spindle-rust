# Temporal Reasoning

Spindle supports timepoint ("as-of") reasoning, allowing you to reason about facts and rules active at a specific moment in time.

## Time Points

Time points are represented as milliseconds since the Unix epoch (UTC).

### Supported Formats

```lisp
(moment "2024-06-15T14:30:00Z")    ; RFC3339 / ISO 8601 string (UTC)
1718461800000                      ; Integer epoch milliseconds
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

## Temporal Atom Identity (v0.3.0)

Starting in v0.3.0 (SPEC-018), temporal atoms with different time bounds are treated as **distinct atoms** during reasoning. Previously, `p[1,10]` and `p[20,30]` collapsed to the same internal identifier, which caused incorrect behaviour such as premature rule firing and cross-window satisfaction.

### How It Works

The internal `AtomKey` used for indexing now includes temporal bounds as part of atom identity. Two literals that differ only in their temporal intervals produce distinct identifiers:

```lisp
; These are now three distinct atoms:
(given (during p 1 10))      ; p[1,10]
(given (during p 20 30))     ; p[20,30]
(given p)                    ; p (base, no temporal bounds)
```

This means a rule that requires `p` in its body will **not** fire when only `p[1,10]` is proven, because they are different atoms. To connect temporal facts to their base predicates, Spindle uses bridging rules (see below).

### What Changed

| Before v0.3.0 | After v0.3.0 |
|---|---|
| `p[1,10]` and `p[20,30]` share the same internal ID | Each gets a distinct ID |
| A rule needing `p` fires when `p[1,10]` is proven (incorrect) | Fires only when `p` (base) is proven |
| Query for `p[1,10]` matches `p[20,30]` (false positive) | Matches only `p[1,10]` exactly |

## Temporal Bridging Rules

Because temporal atoms are now distinct from their base predicates, Spindle automatically generates **synthetic bridging rules** that connect temporal facts to their base forms. This ensures backward compatibility: proving a temporal fact still makes the base predicate available to non-temporal rules.

### Automatic Bridge Generation

Given a temporal fact like `>> p[1,10]`, the `TemporalBridge` pipeline stage generates a strict rule:

```
p[1,10] → p
```

This means: "if `p` holds during `[1,10]`, then `p` holds in general." The bridge is a **strict** rule (not defeasible), because temporal subsumption is a semantic entailment.

### Negation Bridges

For every positive bridge, a coupled negation bridge is also generated:

```
~p[1,10] → ~p
```

This ensures that evidence against a predicate in any temporal window propagates to the base level, enabling correct ambiguity blocking under skeptical semantics.

### Example

Consider a theory about employment:

```lisp
; Alice worked at Acme from 2020 to 2022
(given (during (employed alice acme)
               (moment "2020-01-01T00:00:00Z")
               (moment "2022-01-01T00:00:00Z")))

; If employed, then has_income
(rule r1 (=> (employed alice acme) (has_income alice)))
```

The `TemporalBridge` stage automatically generates:

```lisp
; Synthetic bridge (strict):
; (employed alice acme)[2020,2022] → (employed alice acme)
```

This allows rule `r1` to fire, because the bridge proves the base `(employed alice acme)` from the temporal fact. Without the bridge, `r1` would never fire since its body expects the base atom.

### Bridge Rule Labels

Bridge rules use a reserved label namespace `__bridge::` to avoid collisions with user-defined rules:

- Positive bridge: `__bridge::<literal_spl>`
- Negation bridge: `__bridge::neg::<literal_spl>`

User-defined rules must not use labels starting with `__bridge::`.

### Pipeline Placement

The `TemporalBridge` stage runs after grounding and before temporal variable validation:

```
Validate → WildcardRewrite → Ground → TemporalBridge → TemporalVarValidation → [TemporalFilter]
```

It is enabled by default. For theories with no temporal atoms, the stage is a no-op.

## Temporal-Aware Query Operators (v0.3.0)

The query operators `query`, `what_if`, `why_not`, and `abduce` now use temporal-aware matching when comparing a query literal against conclusions.

### Matching Behaviour

- **Base query** (no temporal bounds): Acts as a wildcard, matching any temporal variant of the predicate. This preserves backward compatibility.
- **Temporal query** (with bounds): Requires an exact temporal match.

```lisp
; Given these conclusions after reasoning:
;   +d p[1,10]
;   +d p[20,30]
;   +d p         (via bridging)

; Querying "p" (base) matches all three — wildcard behaviour
; Querying "p[1,10]" matches only +d p[1,10] — exact temporal match
; Querying "p[5,15]" matches nothing — no exact match exists
```

This prevents false positives where querying `p[1,10]` would previously match `p[20,30]` because temporal bounds were ignored in comparisons.

## Limitations

1.  **Timepoint only**: Currently, Spindle supports reasoning *at* a timepoint. Interval inference (deriving new intervals from rules) is planned for a future release.
2.  **No Allen Relations in SPL**: All 13 Allen relations are implemented in the core library (available via the Rust API), but are not yet exposed in SPL syntax. This requires interval variables (e.g., `(during p ?t)`) which are planned for a future release.
3.  **No Interval Variables**: You cannot bind a variable to a time interval (e.g., `(during p ?t)` is not supported).
