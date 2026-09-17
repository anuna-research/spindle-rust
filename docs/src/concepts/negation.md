# Negation

Spindle uses **strong (explicit) negation**. `(not p)` must be supported by a
fact or rule; absence of a proof of `p` does not establish `(not p)`.

```spl
(given (not guilty))
(normally no-flight penguin (not flies))
```

## Positive evidence and unknown results

| Evidence | Interpretation |
|---|---|
| `+d p` | Positive proof of `p` |
| `+d ~p` | Positive proof of its explicit complement |
| Neither positive tag | Unknown from positive evidence; negative tags may or may not exist |
| Both positive tags | Inconsistent positive evidence, possible with contradictory definite facts |

For example, `(always loop p p)` supplies no positive or negative proof for `p`.
A literal absent from the theory is not automatically listed in the engine's
conclusions. A query can still report its status as unknown.

The negative proof tag `-d p` does not mean `+d ~p`. Read more in
[Conclusions](conclusions.md).

## Conflicts and definite evidence

```spl
(given trigger)
(normally left trigger p)
(normally right trigger (not p))
```

Without priority, these competing defaults block each other. A definite proof of
`~p` prevents a merely defeasible proof of `p`. If `p` is also definitely proved,
both sides retain `+D` and `+d`; inconsistency does not prove unrelated literals.

## Negated premises

```spl
(given (bird eddie))
(given (not (penguin eddie)))
(normally flight
  (and (bird ?x) (not (penguin ?x)))
  (flies ?x))
```

The negative premise needs its own positive proof. Removing the explicit
not-penguin fact leaves the rule without that support.

## Explicit defeasible defaults

To model a presumption that evidence may override, use an empty-body defeasible
rule, not a fact:

```spl
(normally presume () (not guilty))
(normally convict evidence guilty)
(prefer convict presume)
```

Without evidence, this derives `+d ~guilty`. Adding `(given evidence)` lets the
preferred rule derive `+d guilty`. This is an explicit domain default; SPL has no
general negation-as-failure operator. A `(given (not guilty))` fact would remain
definitely true and cannot be overridden this way.
