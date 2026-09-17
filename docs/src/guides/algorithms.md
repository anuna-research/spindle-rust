# Algorithms

Spindle's built-in engine implements traditional ambiguity-blocking **DL(∂)**
with team defeat and four constructive proof tags. `StandardReasoner` is the
backend selected by `select_reasoner("standard")`.

## Proof propagation

Facts establish `+D` and therefore `+d`. Strict rules propagate definite proofs.
Defeasible reasoning combines supported rules, negative definite evidence for
the complement, and checks against opposing rules. A strict rule whose premises
are only defeasibly proved can also contribute defeasible support.

The engine derives negative tags through their own proof conditions. Absence of `+D` or `+d`
is not enough to emit `-D` or `-d`. The engine propagates evidence until no further
tags can be established.

## Conflict handling

```spl
(given trigger)
(normally left trigger p)
(normally right trigger (not p))
```

With no priority, neither side is defeasibly provable: this is ambiguity blocking.
Adding `(prefer left right)` lets `p` win. Under team defeat, different supporting
rules can defeat different attackers; a single rule need not outrank every opponent.
Defeaters can attack the complementary conclusion but cannot establish their own
heads. When a premise is disproved, the engine discards the attacker. An unproved premise alone does not justify discarding it.

## Cycles and inconsistency

```spl
(always strict-loop p p)
(normally defeasible-loop q q)
```

The strict self-loop leaves `p` undecided at both levels. The defeasible self-loop
has `-D q`, but neither `+d q` nor `-d q`. A rule depending on such an undecided
premise can continue to block an opponent.

If both `p` and `(not p)` are facts, both receive `+D` and `+d`. This strict
inconsistency does not establish unrelated literals. See
[Conclusions](../concepts/conclusions.md) for how to read the tags.

## Grounding and aggregation

Grounding instantiates variables before ordinary reasoning. Grounding can grow
combinatorially; its budgets constrain large theories. Aggregate programs
infer strata, complete earlier reasoning, and lower aggregate results with
snapshot premises before continuing. The engine accepts ordinary cycles but rejects cycles through
an aggregate dependency. See [Aggregation](aggregation.md).

## Theoretical complexity and grounding

For propositional theories, defeasible inference runs in linear time relative to theory size (Maher, 2001).
With first-order variables, grounding instantiates rules against known facts before reasoning begins.
As with Datalog, grounding can be exponential in rule body size. Reasoning over the ground theory remains polynomial.
This tractability makes defeasible logic practical where other non-monotonic formalisms are intractable.

## Measuring performance

Indexes, worklists, and bitsets reduce repeated lookup and propagation work.
End-to-end cost also includes grounding, conflict checks, and, for aggregates,
repeated prefix reasoning. The `make bench` and `make bench-aggregation` targets measure representative theories.
A propositional complexity bound does not bound the whole pipeline.
