# Disjunctive Conditions

Defeasible logic does not support disjunction in rule bodies. This is by design, not a missing feature — the proof theory (Nute/Billington/Antoniou) defines derivation over conjunctive rules only. Adding `or` breaks the well-defined defeat and superiority semantics that make defeasible reasoning tractable.

Separate rules with the same head express disjunctive conditions:

```spl
; "If rain or snow, take umbrella"
(normally rain-means-umbrella rain take-umbrella)
(normally snow-means-umbrella snow take-umbrella)
```

Each rule independently supports the same conclusion. If either `rain` or `snow` is provable, `take-umbrella` follows. This standard encoding of disjunctive conditions preserves per-rule defeat. Overriding one path does not affect the other.
