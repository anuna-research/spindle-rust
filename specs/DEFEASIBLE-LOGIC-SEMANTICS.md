# Defeasible Logic — Formal Semantics

The default standard reasoner implements traditional **ambiguity-blocking,
team-defeat DL(∂)** on finite ground, nonmodal theories. The reference proof
conditions are in [Maher et al., *Rethinking Defeasible Reasoning: A Scalable
Approach*, section 2](https://doras.dcu.ie/24726/1/Scalable_Defeasible_Logic.pdf).
Section 4 defines DL(∂∥), a different logic; it is not an interchangeable
implementation of this default.

This document specifies ordinary inference. Temporal-family matching, modal
transformation, arithmetic, and aggregates are extensions around that core.
The independent aggregate source semantics and lowering proofs are described in
[the aggregation guide](../lean/AGGREGATION.md).

## Notation

A theory consists of facts F, rules R, and an acyclic superiority relation >.
Rules have a finite body A(r) and one head. Multiple source heads are normalized
into separate rules. Facts are represented internally as fact rules, but do not
participate in R's attack comparisons: their effects enter through definite tags.

- Rs[q]: strict rules whose head is q.
- Rsd[q]: strict or defeasible rules whose head is q.
- R[q]: strict rules, defeasible rules, and defeaters whose head is q.
- ~q: the classical complement of q.
- P: tags already established by a finite proof.

A rule is applicable at a level when **all** its premises have positive proofs
at that level. It is discarded when **some** premise has a negative proof.
A rule may be neither applicable nor discarded.

## Four constructive proof conditions

Append +D q if:

```text
q ∈ F
OR ∃r ∈ Rs[q]: ∀a ∈ A(r), +D a ∈ P
```

Append -D q if:

```text
q ∉ F
AND ∀r ∈ Rs[q]: ∃a ∈ A(r), -D a ∈ P
```

Append +d q if:

```text
+D q ∈ P
OR (
    ∃r ∈ Rsd[q]: ∀a ∈ A(r), +d a ∈ P
    AND -D ~q ∈ P
    AND ∀s ∈ R[~q]:
        (∃a ∈ A(s), -d a ∈ P)
        OR (∃t ∈ Rsd[q]: (∀a ∈ A(t), +d a ∈ P) AND t > s)
)
```

Append -d q if:

```text
-D q ∈ P
AND (
    (∀r ∈ Rsd[q]: ∃a ∈ A(r), -d a ∈ P)
    OR +D ~q ∈ P
    OR (∃s ∈ R[~q]:
        (∀a ∈ A(s), +d a ∈ P)
        AND ∀t ∈ Rsd[q]:
            (∃a ∈ A(t), -d a ∈ P) OR NOT(t > s))
)
```

The empty universal condition is true. Thus a literal with no fact or strict
support gets -D; if it also has no productive support, it gets -d.
A superior defender that is still undecided cannot be treated as discarded.
Superiority is checked between the named rules; it is not implicitly closed
transitively. Defeaters can block a conclusion but cannot establish it.

## Undecided is distinct from disproved

Neither proof level is total. Missing +D is not -D; missing +d is not -d.

| Theory | Tags for p |
|---|---|
| p -> p | none |
| p => p | -D p only |
| => p; => ~p | -D p, -d p (and the same for ~p) |
| fact p; fact ~p | +D p, +d p (and the same for ~p) |

An unseeded cycle is not automatically disproved. No lambda-based unfoundedness
rule or final negative sweep is part of traditional DL(∂). In particular,
`~q -> ~q; => q` does not prove +d q: the required -D ~q is unavailable.

Definite subsumption is unconditional: **+D q implies +d q**, including when
+D ~q also holds. Strict inconsistency therefore reaches both proof levels.
This does not introduce classical explosion: an unrelated conclusion still
needs a rule or fact that establishes it. For a single literal, a positive and
negative tag at the same level cannot both be derived.

## Aggregation example

```text
=> p       => ~p       p ~> ~q       => q
     conflict               |
       -d p  ---- discards --+
                             +d q
                               |
                           count(q) = 1
```

The negative proof for p discards the attacker. An undecided premise would not
be enough to discard it. A fold reads only positive defeasible rows after the
lower stratum completes. An aggregate-dependent strict rule retains a defeasible
snapshot premise, so it does not automatically acquire a definite proof.

## Executable models and termination

Rust computes definite positive and negative closure, then computes constructive
positive and negative defeasible closure. Reports contain only derived tags.
Each proof bit changes at most once; a productive iteration must add a bit.

`lean/Spindle/Aggregation/Operational.lean` independently implements the four
conditions as a growing finite set of tagged literals. Literals without any
head rule have immediate negative proofs and need no stored state. The other
candidates are the four tags on rule heads. The checked finite bound reaches
a fixed point. Prefix equivalence preserves **all four** tags, including the
absence of a tag when its proof is undecided.

The earlier lambda-only and strengthened two-sided Lean models remain available
for their historical property proofs. They are not the standard or aggregate
conformance oracle. The family oracle expands atemporal body alternatives into
ordinary rules and executes the traditional core; temporal-family matching
remains an explicitly separate extension, not a claim about the original
propositional literature.

## Compatibility change

Earlier releases quarantined strict inconsistency, seeded negative defeasible
proofs from absence in lambda, and reported every remaining positive-proof
failure as a negative tag. Those behaviors have been removed from the default
reasoner. Consumers must distinguish an absent proof from a negative proof.
The CLI fixture and property tests assert the traditional behavior; Lean/Rust
differential suites reject every mismatch in their compared scope.
