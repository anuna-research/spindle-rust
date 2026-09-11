# Literature review: aggregation semantics

Reviewed 2026-09-11 to guide the Lean formalisation and the eventual repair of
PRs #26–29. This is a focused review of primary papers, not a claim that one
standard aggregation semantics exists for all logic languages.

## What the papers establish

**Completed-input stratification is an established design.** Sudarshan and
Ramakrishnan, *Aggregation and Relevance in Deductive Databases* (VLDB 1991),
§2, restrict aggregation dependencies so they cannot participate in recursion.
Their aggregate evaluation collects the relevant derivable tuples before
computing the result. This supports ordering complete aggregate inputs before
their consumers. It does not supply a semantics for Spindle's proof tags.
[Paper](https://www.vldb.org/conf/1991/P501.PDF).

**Recursive aggregation requires additional semantic structure.** Ross and
Sagiv, *Monotonic Aggregation in Deductive Databases* (JCSS 54(1), 1997;
preliminary PODS 1992 version), propose a minimal-model treatment that permits
aggregate-containing components under complete-lattice and monotonicity
conditions. This is a different contract from folding a completed finite
relation. Associativity and commutativity of a reducer alone do not supply
those conditions. The publisher's abstract was available in this review;
the full journal paper was not retrieved.
[Publisher abstract](https://www.sciencedirect.com/science/article/pii/S0022000097914537),
[author bibliography](https://www.cs.columbia.edu/~kar/pubsk/test.html).

**There is no single uncontested semantics for recursive aggregates.** Liu and
Stoller, *Recursive Rules with Aggregation: A Simple Unified Semantics*
(arXiv version 3, 2022), explicitly distinguish the assumptions underlying
several semantics. Their §6 comparisons separate stratified/monotonic programs
from more general recursive cases. Rejecting aggregate cycles is therefore a
deliberate restriction of the language being formalised, not a proof that no
meaning can ever be assigned to such a program.
[Paper](https://arxiv.org/pdf/2007.13053).

**Tuple identity and value multiplicity must be specified separately.**
Mohapatra and Genesereth, *Aggregation in Datalog Under Set Semantics*, §2,
Example 2, preserve distinguishable tuples while aggregating a selected value
component. Equal extracted values can therefore contribute more than once
without counting multiple derivations of the same tuple. This supports the
current whole-row-deduplication-before-extraction design. A shift identifier
must remain part of the row when two otherwise identical shifts must count
separately. Deduplicating extracted pay amounts would change the query.
[Author-hosted paper](https://web.stanford.edu/~abhijeet/papers/aggregates.pdf).

**Defeasible logic cannot simply inherit an ASP aggregate semantics.** Antoniou,
Billington, Governatori, and Maher, *Embedding Defeasible Logic into Logic
Programming* (accepted 2005), §2, distinguish definite proofs, defeasible proofs,
and defeaters. Priorities only affect competing complementary heads; an
unrelated priority pair has no effect on their proof theory. The paper's general
embedding uses Kunen semantics; the stated stable-model equivalence needs an
additional decisiveness condition. These results justify preserving proof tags
and considering all potential attackers, but they do not define an aggregation
extension for Spindle.
[Paper](https://arxiv.org/pdf/cs/0511055).

## Choices for this formalisation

These are Spindle/Skein design choices informed by the papers and the existing
`../skein/nurses-award-spindle/spindle-extensions.md` plan. They are not attributed
to the papers as universal requirements.

| Question | Current contract |
| --- | --- |
| What does a fold read? | Final surviving ground rows from earlier strata, not grounding candidates or alternative derivation trees. |
| How are duplicates treated? | Deduplicate complete rows, then extract values. Equal values from distinct rows retain multiplicity. |
| What defines a group? | Already-bound outer variables restrict the matching relation. The finite lowerer enumerates outer bindings and keeps fold-local matching variables separate. |
| What does empty mean? | An explicit seed is returned on empty input; `required` fails. For nonempty input the seed participates once. |
| Can aggregation be recursive? | The initial language rejects every dependency cycle containing a fold. Ordinary positive recursion remains permitted. |
| Does strong negation add a strict dependency? | No. `not p` in Spindle is a complementary literal, not negation-as-failure. It uses an ordinary dependency; conflict resolution settles the competing heads together. |
| Where are competing rules placed? | Every producer/attacker of a predicate domain shares a stratum, including defeaters. All heads of one rule share its stratum. |
| How are modes treated? | Conservatively collapse mode and polarity for scheduling by predicate name/arity. This can reject programs that a more precise mode-sensitive analysis would accept; it does not silently omit possible conflicts. |
| Do unrelated priority pairs add graph edges? | No. Related competing heads already share a domain. Unknown referenced labels are rejected by the schema recognizer. |
| What happens to a transported `+d`? | It stays defeasible. Completion of its stratum is not a definite proof. |

The graph now has rule nodes as well as predicate nodes. This avoids assuming,
without checking, that a multi-head rule's outputs were assigned the same stage:

```text
ordinary input ------0------> rule
aggregate input -----1------> rule
                              | ^
                            0 | | 0
                              v |
                         each head domain
```

Every edge requires `stage(source) + weight <= stage(target)`. A zero-weight
edge in both directions fixes the rule and head domain to the same stage.

## Additional boundary requirement from the implementation

The local three-phase reasoner uses lambda support to decide whether an attack
can reach its target (`Closure.attackReaches`). A lower literal can remain in
lambda while conflict prevents its defeasible proof. Positive conclusion
transport alone therefore does not preserve ordinary attack applicability.
This is an observation about Spindle's implementation, not a new claim from
the papers above. Kernel-checked examples in `GroundBackendTests.lean` show an
ambiguous lower `p` keeping a later defeater against `q` applicable; dropping
that potential support incorrectly permits `q`.

The reference ground backend consequently retains the complete lower rule
prefix, including priorities and defeaters, and recomputes all three closures.
This preserves the information needed by the local computation without treating
lambda-only rows as aggregate inputs. A more compact boundary representation
will need a separate equivalence proof. The earlier transport theorem only
establishes rule type preservation, not semantic equivalence.

## Questions still requiring an evaluator contract

**Proof strength of a newly computed aggregate.** Preserving an existing `+d`
across a boundary is settled. Assigning a tag to a *new* aggregate-derived
conclusion is not. In particular, an exact sum depends on the input relation's
completeness, not merely on positive proofs for the included rows. Even if every
included row has a definite proof, adding another input fact can change the sum.
We must not claim ordinary SDL `+D` monotonicity for such results without defining
how relation closure enters the proof theory. The execution API now states this boundary explicitly: `foldAt` returns a
snapshot-relative value without an SDL proof tag, and later checked stages
cannot revise that snapshot. The finite lowerer now offers an explicit conservative policy: add a fresh
defeasible closure premise while retaining the original rule kind. Alternatively,
reject aggregate-bearing strict rules. The prefix-equivalence theorem compares
execution with the final theory under the selected policy; it does not attribute
this policy to the literature as a universal standard.

**Arithmetic and errors.** The current reducer laws concern total mathematical
operations. Machine overflow, finite-precision rounding, and expression failures
need a separate refinement/error contract. Neither a fold-safe flag nor a
stratum assignment proves those laws for a runtime operation. Exact accumulation
followed by checked conversion is a candidate design, not yet an implemented
or proved one.

**Termination.** Stratification orders aggregate computations; it does not by
itself establish a finite value domain for ordinary recursive arithmetic rules.
The executable lowerer now grounds over a declared finite domain and errors
when an aggregate result lies outside it. All ground reasoning phases complete
within their proved finite-literal bounds. Supporting unbounded generated values
still needs a separate finiteness or resource-limit contract.

The current scope and verified modules are described in [AGGREGATION.md](AGGREGATION.md).
