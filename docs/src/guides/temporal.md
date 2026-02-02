# Temporal Reasoning

Spindle supports temporal reasoning using Allen's interval algebra.

## Time Points

### Basic Moments

```lisp
(moment 2024)                      ; Year only
(moment 2024 6 15)                 ; Year, month, day
(moment 2024 6 15 14 30 0 "UTC")   ; Full timestamp
(moment "2024-06-15T14:30:00Z")    ; ISO 8601 string
```

### Special Values

```lisp
inf    ; Positive infinity (never ends)
-inf   ; Negative infinity (always existed)
```

## During Operator

The `during` operator associates a literal with a time interval:

```lisp
(given (during bird 1 10))
(given (during (employed alice acme) (moment 2020) (moment 2023)))
```

Syntax: `(during literal start end)`

### Example: Employment History

```lisp
; Alice worked at Acme from 2020 to 2022
(given (during (employed alice acme) (moment 2020) (moment 2022)))

; Alice works at Beta from 2022 onwards
(given (during (employed alice beta) (moment 2022) inf))

; Employees can access company resources
(normally r1 (during (employed ?x ?company) ?t) (during (can-access ?x ?company) ?t))
```

## Allen's Interval Relations

Spindle supports all 13 Allen interval relations for comparing time intervals.

### Basic Relations

```lisp
(before ?t1 ?t2)    ; t1 ends before t2 starts
                    ; |--t1--|
                    ;           |--t2--|

(after ?t1 ?t2)     ; t1 starts after t2 ends
                    ;           |--t1--|
                    ; |--t2--|

(meets ?t1 ?t2)     ; t1 ends exactly when t2 starts
                    ; |--t1--|
                    ;        |--t2--|

(met-by ?t1 ?t2)    ; t1 starts exactly when t2 ends
                    ;        |--t1--|
                    ; |--t2--|
```

### Overlap Relations

```lisp
(overlaps ?t1 ?t2)      ; t1 starts before t2, ends during t2
                        ; |----t1----|
                        ;       |----t2----|

(overlapped-by ?t1 ?t2) ; t2 starts before t1, ends during t1
                        ;       |----t1----|
                        ; |----t2----|
```

### Containment Relations

```lisp
(contains ?t1 ?t2)  ; t1 strictly contains t2
                    ; |--------t1--------|
                    ;     |--t2--|

(within ?t1 ?t2)    ; t1 is strictly within t2
                    ;     |--t1--|
                    ; |--------t2--------|
```

### Boundary-Sharing Relations

```lisp
(starts ?t1 ?t2)      ; Same start, t1 ends first
                      ; |--t1--|
                      ; |------t2------|

(started-by ?t1 ?t2)  ; Same start, t2 ends first
                      ; |------t1------|
                      ; |--t2--|

(finishes ?t1 ?t2)    ; t1 starts after t2, same end
                      ;       |--t1--|
                      ; |------t2----|

(finished-by ?t1 ?t2) ; t2 starts after t1, same end
                      ; |------t1----|
                      ;       |--t2--|

(equals ?t1 ?t2)      ; Same start and end
                      ; |----t1----|
                      ; |----t2----|
```

## Temporal Rules

### Propagating Temporal Properties

```lisp
; If something is a bird during interval t, it has feathers during t
(normally r1 (during bird ?t) (during has-feathers ?t))
```

### Temporal Constraints

```lisp
; Events that precede others
(normally r1
  (and (during event1 ?t1)
       (during event2 ?t2)
       (before ?t1 ?t2))
  event1-precedes-event2)
```

### Overlapping Intervals

```lisp
; If two meetings overlap, there's a conflict
(normally conflict
  (and (during (meeting ?room ?m1) ?t1)
       (during (meeting ?room ?m2) ?t2)
       (overlaps ?t1 ?t2))
  (scheduling-conflict ?room))
```

## Example: Project Timeline

```lisp
; Project phases
(given (during (phase design) (moment 2024 1 1) (moment 2024 3 31)))
(given (during (phase implementation) (moment 2024 3 1) (moment 2024 8 31)))
(given (during (phase testing) (moment 2024 7 1) (moment 2024 9 30)))

; Phases that overlap
(normally r1
  (and (during (phase ?p1) ?t1)
       (during (phase ?p2) ?t2)
       (overlaps ?t1 ?t2))
  (parallel-work ?p1 ?p2))

; Resource needed during phase
(normally r2 (during (phase implementation) ?t) (during needs-developers ?t))
```

## Querying at Specific Times

To query conclusions at a specific time, filter the results by temporal bounds.

```lisp
; Is alice employed at acme in 2021?
; Check: (during (employed alice acme) ?t) where 2021 is within ?t
```

## Limitations

1. **Discrete time**: Moments are discrete, not continuous
2. **No duration arithmetic**: Cannot compute `t1 + 5 days`
3. **No fuzzy boundaries**: Intervals have precise start/end
4. **Grounding required**: Temporal variables must be bound by facts

## Best Practices

1. **Use meaningful intervals**
   ```lisp
   ; Good: named intervals
   (given (during project-alpha (moment 2024 1 1) (moment 2024 12 31)))

   ; Avoid: magic numbers
   (given (during x 1 365))
   ```

2. **Document temporal semantics**
   ```lisp
   ; Employment is during working hours only
   (meta r1 (note "Applies during business hours"))
   ```

3. **Consider open intervals**
   ```lisp
   ; Ongoing employment (no end date)
   (given (during (employed alice acme) (moment 2024) inf))
   ```
