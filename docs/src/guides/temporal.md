# Temporal Reasoning

Spindle supports bounded temporal literals, whole-interval variables, Allen
constraints, and “as-of” filtering when requested.

## Time points and intervals

Time points are milliseconds since the Unix epoch (UTC). Accepted bounds include integers,
`(moment "2024-06-15T14:30:00Z")`, or `-inf` / `inf`. Multi-argument calendar forms
such as `(moment 2024 6 15)` are unsupported.

```spl
(given (during (employed alice acme) 100 200))
(given (during (employed alice beta) 201 inf))
```

The explicit endpoint form is `(during literal start end)`. Concrete bounds are
inclusive for active-at filtering.

## Interval variables and constraints

The premise `(during literal ?T)` binds a whole interval:

```spl
(given (during (p a) 1 10))
(given (during (q b) 20 30))
(normally sequence
  (and (during (p ?x) ?T)
       (during (q ?y) ?S)
       (before ?T ?S))
  (ordered ?x ?y))
```

This derives `(ordered a b)`. SPL supports all 13 Allen constraints:

- `before`, `after`, `meets`, `met-by`
- `overlaps`, `overlapped-by`, `starts`, `started-by`
- `within`, `contains`, `finishes`, `finished-by`, `equals`
`within` names Allen's During relation, avoiding collision with SPL's `during`
literal wrapper. Constraints filter interval bindings during grounding.

Intervals can also be carried into rule heads:

```spl
(given (during (p a) 1 10))
(normally copy (during (p ?x) ?T) (during (q ?x) ?T))
```

This derives `q(a)` with bounds `[1,10]`. Endpoint variables in
`(during literal ?start ?end)` are also supported. Temporal variables cannot be
used as numeric arithmetic operands. Default preparation validation rejects unresolved temporal expressions.

`active-at`, `past-at`, and `future-at` are also available as body constraints,
for example `(active-at ?T 150)` after binding `?T`.

## Exact atoms and family matching

Temporal bounds are part of indexed atom identity: `p@[1,10]`, `p@[20,30]`, and
atemporal `p` are distinct. An atemporal body premise can nevertheless consume
positive evidence from a temporal member of its family:

```spl
(given (during p 1 10))
(always consume p q)
```

This establishes `q` without creating a positive atemporal `p` conclusion.
The current engine uses family-aware body matching and projection tokens;
it does not insert synthetic `TemporalBridge` rules or reserve `__bridge::`
labels for such a stage. Repeated members satisfying one body slot count once.

## Query matching

Atemporal queries use family matching. Bounded queries require identical windows:
a goal for `p@[1,10]` does not match `p@[20,30]`, atemporal `p`, or even the
containing window `p@[0,20]`. The same distinction applies to `requires`,
`what_if`, and `abduce`; `why_not` diagnoses the grounded goal.

The CLI accepts temporal literals in SPL form:

```sh
spindle query '(during p 1 10)' theory.spl --json
```

Explicit family-wide matching in Rust uses
`query_with_match_mode(&theory, &goal, QueryMatchMode::Family)`.

## As-of filtering

```sh
spindle reason theory.spl --at '2026-09-17T12:00:00Z'
```

Preparation keeps rules whose own interval, head literals, and logical body
literals are active at the reference point. Arithmetic premises have no temporal
window. Filtering runs around grounding to check bound temporal expressions. Without `--at`, preparation does not filter temporal evidence to “now”.

Temporal reasoning and aggregation are currently separate supported paths:
the aggregate bridge rejects temporal constructs and temporal preparation options.
