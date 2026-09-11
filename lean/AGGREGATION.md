# Aggregation formalisation

The initial model defines aggregation over a finite, completed relation. It was
started while reviewing PRs #26, #27, and #29 against Skein's requirements. It
specifies the stratum boundary before changing the Rust evaluator.

For example, if two surviving shifts each contribute 25, their sum is 50. Two
proofs of the same shift still contribute only 25. A defeated shift contributes
nothing. A consumer waits until all producers and conflict resolution for the
input relation have completed. Carrying a defeasible conclusion to the next
stratum does not make it definitely provable.

## Definitions and proved properties

| Module | Contract and evidence |
| --- | --- |
| `Spindle/Aggregation/Fold.lean` | A total reducer supplies proofs of associativity and commutativity. `eval_permutation` proves enumeration order cannot change its result. `required_fails_iff_empty` characterizes failure on empty input. `values` deduplicates complete rows before extraction; `aggregate_permutation` covers the combined selection/extraction/fold operation. |
| `Spindle/Aggregation/Stratification.lean` | Ordinary dependencies have weight zero, aggregate dependencies weight one. `path_bound` propagates constraints through both kinds of edge. `no_aggregate_cycle` excludes every cycle containing an aggregate edge. `aggregate_input_finalized` requires aggregate inputs to belong to an earlier stratum. |
| `Spindle/Aggregation/Snapshot.lean` | `snapshot` selects positive conclusions and deduplicates literals. `unproved_excluded` excludes rows with no positive proof. `carry_fact_iff` permits a transported fact rule only for an original `+D` conclusion. `aggregate_frozen` proves that adding current/future-stratum rows cannot change an aggregate's frozen input view. |
| `Spindle/Aggregation/Syntax.lean` | Typed schema fragment with terms, modes, rule kinds, multiple heads, pure expressions, and folds. `applySubst_key` proves that argument grounding preserves the predicate/arity domain. |
| `Spindle/Aggregation/Dependencies.lean` | Extracts ordinary/fold input edges and connects each rule to all head domains. `dependencies_valid_iff` proves extraction is sound and complete for the schema scheduling constraints. `fold_waits_for_producer` covers every producer and attacker sharing the input domain. `checkProgram_iff` verifies the executable certificate checker. |
| `Spindle/Aggregation/Inference.lean` | Automatic inference is sound, complete, and returns the pointwise least valid assignment. Rejection is equivalent to absence of a valid assignment; edge order cannot change the result. `inferProgram` connects these guarantees to schema scheduling. |
| `Spindle/Aggregation/DependencyTests.lean` | Kernel-checked examples exercise extraction and elementary inference. Runtime assertions also cover inferred chains, cycle rejection, defeaters, multiple heads, rule reordering, unrelated priorities, and unsupported schemas. |

Concrete reducer instances cover exact integer sum, minimum, and maximum. Count
uses sum with constant extraction of one. Kernel-checked examples exercise equal
contributions from distinct rows, duplicate rows, grouping, required min/max,
empty inputs, positive recursion, an indirect aggregate cycle, and `+d` transport.

An explicit seed participates once in the fold, even for nonempty input. It is
not required to satisfy an identity law: this preserves the proposed SPL `fold`
syntax's explicit initial-value semantics. `none` represents `required` and uses
the first collected value as the accumulator.

Rows may be structured values. Grouping is represented by a selection function
that closes over an already-bound outer environment. This model does not yet
define a variable-binding language or infer implicit groups.

## Relation to existing proofs

The previous Spindle arithmetic AST has no aggregate or extension-call variant;
its grounding proofs range over literal bodies and a supplied finite domain.
Those proofs do not establish termination or correctness of aggregation.
The existing `pipeline_soundness` theorem concerns diagnostic/repair query
composition, not the prepare/stratify/reason implementation.

The new snapshot bridge uses Spindle's existing `Conclusion` and `Rule` types.
It deliberately preserves the distinction between `+D` and `+d`. The transport
theorem establishes the emitted rule type, not equivalence of reasoning over
the reconstructed theory.

Skein's `language/core.md` §6–7 and `impl/Accounting/{Strata,Eval}.lean` provide
the relevant completed-stratum design: include ordinary dependencies, finish
defeasible filtering, then freeze the relation. Skein's source/key-based
defeasibility differs from Spindle's complementary-literal SDL, so its evaluator
cannot simply be substituted for Spindle's reasoner.

The [literature review](AGGREGATION-LITERATURE.md) separates the established
stratified approach from alternative recursive-aggregate semantics and records
Spindle-specific choices. In particular, strong negation remains an ordinary
dependency, and unrelated superiority pairs add no dependency edges.

## Dependency extraction and its scope

The new extractor operates on typed rule schemas, before argument grounding.
Each rule has a node, and each predicate name/arity has a domain node. Every
head is connected to its rule in both directions with zero-weight edges. This
places all heads, producers, and potential attackers of that domain together.
Each ordinary premise adds a zero-weight edge into the rule; each fold pattern
adds a weight-one edge. Defeaters participate even though they cannot derive
positive conclusions. Arithmetic binds/comparisons introduce no relation edges.

The independent `RuleScheduled` proposition states the required input ordering
and head alignment directly over schema syntax. Its equivalence with graph
validity proves the extractor has not omitted an occurrence. An accepted
certificate therefore establishes these constraints for every schema, not just
for whatever edges happened to be extracted.

This is a conservative conflict analysis: polarity, argument values, and modes
do not split a predicate/arity domain. More precise analysis could accept more
programs. `extractDependencies` rejects schematic/wildcard predicate names,
empty names, duplicate rule labels, and unknown priority references. It does
not silently treat an unknown dynamic predicate as a distinct static relation.
The scope recognizer is not a complete SPL parser or semantic validator.

## Automatic stratum inference

`inferProgram` recognizes the schema fragment, extracts dependencies, and returns
its smallest valid stage assignment. It distinguishes unsupported schemas from
recognized programs that admit no valid assignment. Ordinary recursion can stay
within one stage; every fold requires a strictly earlier input stage.

The reference solver starts all nodes at zero and repeatedly raises targets to
satisfy their incoming edge constraints. Each round caches the computed values.
For a node list of length N (including repeated endpoints), N² rounds suffice:
any valid assignment can be compressed to ranks at most N, while every unsuccessful
round strictly increases the sum of node stages. The completeness proof therefore
rules out rejecting a schedulable graph because of an arbitrary fuel limit.
Soundness and leastness also prove that reordering edges cannot change the result.

This is a verified reference algorithm. An optimized SCC implementation and
cycle-witness diagnostics would be separate refinements.

## Remaining work

1. Connect the typed schema fragment to actual SPL/Rust parsing and grounding.
   Extend recognition and dependency extraction to temporal syntax, schematic
   predicate names, and arithmetic arguments inside literal patterns. The current
   extraction proof covers the explicitly represented static-predicate fragment;
   it does not prove that a Rust-to-Lean translation preserves every construct.
2. Compose per-stratum fixed-point reasoning with the snapshot/aggregate model;
   prove finalization, termination, and preservation of proof strength for that
   evaluator. Final conclusions and correct row ownership are currently inputs
   to the proved boundary properties. The proof strength of a newly computed
   aggregate (as opposed to a transported conclusion) still needs an explicit
   closure/evidence contract; see the literature review.
3. Specify partial extraction and reducer errors, bounded arithmetic, decimals,
   and extension values. Exact integer addition inhabits the reducer contract;
   checked integer addition and floating-point addition cannot be assumed to
   satisfy it. Merely marking Rust `+` as fold-safe does not prove determinism.
4. Add a Lean oracle and Rust differential tests for the repaired implementation,
   including CLI entry points, limits, temporal filtering, and provenance.

These are integration obligations, not claims already discharged by this model.
The model makes no Rust conformance or whole-language termination claim.

## Verification

Run from the repository root:

```sh
scripts/check-lean-verification.sh
```

The root `Spindle.lean` imports the aggregation modules, and `AxiomAudit.lean` audits
their main results. They are included in the normal build and vacuity gate.

See [the PR review](../docs/reviews/pr-26-27-29.md) for the observed Rust failures
that motivated these obligations.
