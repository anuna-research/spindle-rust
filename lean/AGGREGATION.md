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

## Remaining work

1. Define rule-level aggregate syntax and complete dependency extraction,
   including modal conflicts, superiority, and all producers of a relation.
   The current graph assumes relations identify complete conflict domains;
   graph construction is not proved.
2. Implement stratum inference and prove its soundness/completeness against
   `ValidStrata`. The current model verifies a supplied assignment.
3. Compose per-stratum fixed-point reasoning with the snapshot/aggregate model;
   prove finalization, termination, and preservation of proof strength for that
   evaluator. Final conclusions and correct row ownership are currently inputs
   to the proved boundary properties.
4. Specify partial extraction and reducer errors, bounded arithmetic, decimals,
   and extension values. Exact integer addition inhabits the reducer contract;
   checked integer addition and floating-point addition cannot be assumed to
   satisfy it. Merely marking Rust `+` as fold-safe does not prove determinism.
5. Add a Lean oracle and Rust differential tests for the repaired implementation,
   including CLI entry points, limits, temporal filtering, and provenance.

These are integration obligations, not claims already discharged by this model.
The model makes no Rust conformance or whole-language termination claim.

## Verification

Run from the repository root:

```sh
scripts/check-lean-verification.sh
```

The root `Spindle.lean` imports all three modules, and `AxiomAudit.lean` audits
their main results. They are included in the normal build and vacuity gate.

See [the PR review](../docs/reviews/pr-26-27-29.md) for the observed Rust failures
that motivated these obligations.
