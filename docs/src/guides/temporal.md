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

## Limitations

1.  **Timepoint only**: Currently, Spindle supports reasoning *at* a timepoint. Interval inference (deriving new intervals from rules) is planned for a future release.
2.  **No Allen Relations in SPL**: All 13 Allen relations are implemented in the core library (available via the Rust API), but are not yet exposed in SPL syntax. This requires interval variables (e.g., `(during p ?t)`) which are planned for a future release.
3.  **No Interval Variables**: You cannot bind a variable to a time interval (e.g., `(during p ?t)` is not supported).

---

## Date and Time Types

Spindle provides five temporal value types for calendar and clock reasoning. These are distinct from the `during`/`moment` system above — they represent typed values that can appear as predicate arguments, in comparisons, and in `bind` expressions.

| Type | Prefix | What it represents | Example |
|------|--------|--------------------|---------|
| Date | `#d:` | A calendar date (no time) | `#d:2025-07-15` |
| Time | `#t:` | Time of day (minute precision) | `#t:09:00` |
| Datetime | `#dt:` | A timezone-aware instant | `#dt:2025-07-15T14:00+10:00` |
| Duration | `#dur:` | A signed span of time | `#dur:8h30m` |
| Offset | `#off:` | UTC offset for constructing Datetimes | `#off:+10:00` |

**Date** is a pure calendar date with no time or timezone. Use it for scheduling, leave accrual, and calendar-based rules.

**Time** represents a clock reading at minute precision (0–1439 minutes since midnight). Use it for shift classification (e.g., "is this a night shift?").

**Datetime** is a full instant — a date, time, and UTC offset combined. Two Datetimes at different offsets that represent the same moment are equal. Use it when you need to compare or measure across timezones.

**Duration** is a signed span in minutes. Duration components are `d` (1440 min), `h` (60 min), `m` (1 min). `#dur:1d6h30m` = 1×1440 + 6×60 + 30 = 1830 minutes. Equal totals are equal: `#dur:2h` = `#dur:120m`.

**Offset** represents a fixed UTC offset. `#off:Z` is UTC (offset 0). `#off:+10:00` is AEST. Use it with the `datetime` constructor.

### Temporal Literals in Facts and Rules

Temporal values appear anywhere a term can — as fact arguments, in rule heads, and in rule bodies:

```lisp
(given (shift-date alice #d:2025-07-15))
(given (shift-start alice #t:09:00))
(given (shift-end alice #t:17:00))
(given (meeting-dur #dur:1h30m))
```

## Temporal Functions

Temporal functions are called inside `bind` expressions in rule bodies.

### Construction: Building a Datetime

The `datetime` function combines a Date, Time, and Offset:

```lisp
(bind ?dt (datetime #d:2025-07-15 #t:14:00 #off:+10:00))
; => #dt:2025-07-15T14:00+10:00
```

### Extraction: Pulling Apart Values

```lisp
(bind ?d (date-of ?dt))                  ; => #d:2025-07-15
(bind ?t (time-of ?dt))                  ; => #t:14:00
(bind ?dow (day-of-week #d:2025-07-14))  ; => :monday
(bind ?y (year-of #d:2025-12-25))        ; => 2025
(bind ?m (month-of #d:2025-12-25))       ; => 12
(bind ?day (day-of-month #d:2025-12-25)) ; => 25
```

`day-of-week`, `year-of`, `month-of`, and `day-of-month` also accept Datetime arguments — they extract the local date component first.

### Differences: Measuring Spans

```lisp
(bind ?days (days-between #d:2025-07-01 #d:2025-07-15))     ; => 14
(bind ?mins (minutes-between ?dt-start ?dt-end))             ; => signed integer
(bind ?hrs  (hours-between ?dt-start ?dt-end))               ; => signed decimal
```

The sign convention is `second − first`: positive when the second argument is later.

To inspect a Duration value:

```lisp
(bind ?h (duration-hours #dur:1h30m))    ; => 1.5 (Decimal)
(bind ?m (duration-minutes #dur:1h30m))  ; => 90 (Integer)
```

### Calendar Arithmetic: Months and Years

`add-months` and `add-years` move a date forward or backward by calendar months/years. When the target month has fewer days than the source, the day is **clamped** to the last valid day:

```lisp
(bind ?d (add-months #d:2025-01-31 1))   ; => #d:2025-02-28 (clamped from 31)
(bind ?d (add-years #d:2024-02-29 1))    ; => #d:2025-02-28 (2025 is not a leap year)
```

`months-between` and `years-between` count **complete** calendar months/years:

```lisp
(bind ?m (months-between #d:2025-01-31 #d:2025-02-28))  ; => 0 (Feb has no 31st)
(bind ?y (years-between #d:2023-03-01 #d:2025-03-01))   ; => 2
```

## Temporal Arithmetic

The standard arithmetic operators `+`, `-`, `*`, `/` are overloaded for temporal types.

### Addition (`+`)

| Left | Right | Result | Notes |
|------|-------|--------|-------|
| Datetime | Duration | Datetime | Advance by duration |
| Date | Duration | Date | Duration must be whole days |
| Duration | Duration | Duration | Sum of durations |

Addition is commutative — `Duration + Datetime` also works.

```lisp
(bind ?end (+ ?start-dt #dur:8h))         ; Datetime + Duration → Datetime
(bind ?total (+ #dur:2h #dur:1h30m))      ; Duration + Duration → #dur:3h30m
```

### Subtraction (`-`)

| Left | Right | Result | Notes |
|------|-------|--------|-------|
| Datetime | Duration | Datetime | Retreat by duration |
| Datetime | Datetime | Duration | Signed difference in minutes |
| Date | Duration | Date | Duration must be whole days |
| Date | Date | Integer | Signed days |
| Duration | Duration | Duration | Difference |

```lisp
(bind ?diff (- ?dt-end ?dt-start))        ; Datetime − Datetime → Duration
(bind ?days (- #d:2025-07-15 #d:2025-07-01))  ; Date − Date → 14
```

### Scaling (`*` and `/`)

Duration can be scaled by a numeric factor:

```lisp
(bind ?double (* #dur:4h 2))             ; => #dur:8h
(bind ?half (/ #dur:4h 2))               ; => #dur:2h
(bind ?ratio (/ #dur:8h #dur:2h))        ; Duration / Duration → 4 (Decimal)
```

`Duration * Duration` is an error. `Number / Duration` is an error.

### `min` and `max`

`min` and `max` work with temporal values of the **same type**:

```lisp
(bind ?earliest (min ?date-a ?date-b))
(bind ?latest-dt (max ?dt1 ?dt2 ?dt3))
```

Mixing types (e.g., `(min ?date ?time)`) is an error.

## Comparisons

The comparison operators `<`, `>`, `<=`, `>=` work with temporal values. Both operands must be the **same type**:

```lisp
(< ?time #t:06:00)           ; Is this before 6 AM?
(>= ?date #d:2025-01-01)     ; Is this date in 2025 or later?
```

Datetime comparisons are by **instant** — they compare the UTC-equivalent moment, so cross-timezone comparisons work correctly:

```lisp
; AEST 09:00 (UTC 23:00 previous day) is before SGT 09:00 (UTC 01:00)
(given (ev a #dt:2025-07-15T09:00+10:00))
(given (ev b #dt:2025-07-15T09:00+08:00))
(normally r1
  (and (ev a ?dt-a) (ev b ?dt-b) (< ?dt-a ?dt-b))
  (a-before-b))
; => a-before-b is derived
```

## Worked Example: Shift Scheduling

A complete example combining temporal facts, extraction, arithmetic, and comparisons:

```lisp
; Facts: employee shift data
(given (shift-date alice #d:2025-07-14))
(given (shift-start alice #t:05:00))
(given (shift-end alice #t:13:00))
(given (shift-date bob #d:2025-07-14))
(given (shift-start bob #t:09:00))
(given (shift-end bob #t:17:00))

; Rule 1: classify early shifts (start before 06:00)
(normally r-early
  (and (shift-start ?who ?time) (< ?time #t:06:00))
  (early-shift ?who))

; Rule 2: extract day of week from shift date
(normally r-day
  (and (shift-date ?who ?date)
       (bind ?day (day-of-week ?date)))
  (works-on ?who ?day))

; Rule 3: compute shift duration
(normally r-dur
  (and (shift-start ?who ?start)
       (shift-end ?who ?end)
       (bind ?dur-mins (- ?end ?start))
       (bind ?dur-hrs (/ ?dur-mins 60)))
  (shift-hours ?who ?dur-hrs))

; Results:
; early-shift(alice) — alice starts at 05:00
; works-on(alice, :monday) — 2025-07-14 is Monday
; works-on(bob, :monday)
; shift-hours(alice, 8)  — 13:00 - 05:00 = 480 min = 8 hrs
; shift-hours(bob, 8)    — 17:00 - 09:00 = 480 min = 8 hrs
```

### Fold with Durations

Duration values work with `fold` for aggregation:

```lisp
(given (task a #dur:2h))
(given (task b #dur:1h30m))
(given (task c #dur:45m))

(normally r-total
  (fold ?sum #dur:0m + ?d (task ?name ?d))
  (total-duration ?sum))
; => total-duration(#dur:4h15m)  (120 + 90 + 45 = 255 minutes)
```

## Restrictions

| Restriction | Example | Error |
|-------------|---------|-------|
| Comparisons require same type | `(< #d:2025-01-01 #t:09:00)` | Type error |
| Date ± Duration requires whole days | `(+ #d:2025-01-01 #dur:2h)` | Duration must be multiple of 1440 minutes |
| No Date + Time cross-type arithmetic | `(+ #d:2025-01-01 #t:09:00)` | Type error — use `datetime` constructor |
| Duration × Duration not supported | `(* #dur:2h #dur:3h)` | Type error |
| Number / Duration not supported | `(/ 10 #dur:2h)` | Type error |
| `min`/`max` require same temporal type | `(min #d:2025-01-01 #t:09:00)` | Type error — mixed temporal types |

When any of these occur during grounding, the substitution is discarded — the rule does not fire for that ground instance.
