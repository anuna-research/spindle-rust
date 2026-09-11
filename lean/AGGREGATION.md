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
| `Spindle/Aggregation/Execution.lean` | A finite stage driver checks batch ownership and preserves tagged conclusions verbatim. `runStages_old_conclusion_iff` prevents later changes to evidence for earlier-owned literals; `runStages_preserves_fold` proves frozen aggregate observations survive subsequent execution. |
| `Spindle/Aggregation/ExecutionTests.lean` | Kernel-checked examples cover multiple stages, reading prior results, deduplication, negative-row exclusion, wrong-owner rejection, backend errors, empty folds, and snapshot-relative exact sums. |
| `Spindle/Aggregation/GroundBackend.lean` | Re-runs each retained finite ground rule prefix through the existing reasoner. `groundBackend_completed` proves each batch comes from finalized delta/lambda/partial closures; `groundBackend_runs` proves recognized inputs finish every requested finite stage count. |
| `Spindle/Aggregation/GroundBackendTests.lean` | Kernel-checked regressions cover ambiguous lower support, attack reachability, original priorities, proof strength, ordinary recursion, defeated aggregate rows, and rejected schedules. |
| `Spindle/Aggregation/PrefixEquivalence.lean` | `reason_agrees_on_closed_domains` proves agreement of delta, lambda, and partial on dependency-closed domains. `reasonedStage_equivalent` proves exact tagged-batch equality; `executeGround_membership` covers accumulated execution. |
| `Spindle/Aggregation/Lowering.lean` | Grounds typed schemas over a finite domain and lowers folds stage by stage. Atom encoding is injective on the table and cannot collide with closure guards. `lowerStages_completed_agree` preserves completed reasoning during lowering. `evaluateProgram_correct` proves scheduled execution has exactly the full lowered theory's tagged conclusions. |
| `Spindle/Aggregation/LoweringTests.lean` | Runtime integration assertions cover inference, grounding, grouping, duplicate rows, defeated rows, priorities, multiple heads, chained folds, evidence policies, empty inputs, and explicit scope/domain errors. |

Concrete reducer instances cover exact integer sum, minimum, and maximum. Count
uses sum with constant extraction of one. Kernel-checked examples exercise equal
contributions from distinct rows, duplicate rows, grouping, required min/max,
empty inputs, positive recursion, an indirect aggregate cycle, and `+d` transport.

An explicit seed participates once in the fold, even for nonempty input. It is
not required to satisfy an identity law: this preserves the proposed SPL `fold`
syntax's explicit initial-value semantics. `none` represents `required` and uses
the first collected value as the accumulator.

Rows may be structured values. Grouping is represented by a selection function
that closes over an already-bound outer environment. The finite lowerer enumerates
outer bindings over a declared domain, then matches each fold row with a fresh
extension of that binding. Head/ordinary-premise variables and explicit grouping
variables remain outer; variables used only inside a fold pattern remain local.

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

## Checked stage execution

`executeStages` starts from stage zero with an empty conclusion list. At each
stage it passes the prior tagged conclusions to a local backend. A returned
batch is committed only if every literal belongs to that stage. Backend errors,
including resource limits, propagate as errors. Empty batches still advance the
stage. The generic driver accepts a stage count and literal ownership map. The schema
entry point now obtains both from inference, and checks that the count covers
every literal in the lowered theory.

```text
prior tagged conclusions --> local backend --> ownership check --> append batch
          |                                            |
          +--> frozen positive rows --> fold value     +--> next stage
```

The proofs establish that successful execution preserves earlier tagged evidence
exactly and cannot change a fold over a boundary already reached. The driver
does not infer that an arbitrary successful callback completed local reasoning.
The finite ground reference backend described below now discharges local
completion for ordinary theories; arbitrary callbacks do not inherit that proof.

`foldAt` returns an optional aggregate value relative to a frozen snapshot. It
emits no SDL proof tag. Examples show why this distinction matters: adding a
second definitely proved input row changes an exact sum from 25 to 50. An
all-`+D` input membership check alone cannot establish stable aggregate evidence.
The lowerer consumes that evidence through the explicit closure-guard policy
below. This introduces no ordinary SDL definite axiom for relation closure.

## Finite ground reference backend

`groundBackend` retains every original rule whose head belongs to the current or
an earlier stage, together with the original superiority relation. It re-runs
`reason` over this prefix and publishes only the current stage's conclusions.
The new `Properties.reason_fixedpoints` theorem proves that all three returned
closures are unchanged by a further step, using the existing finite-literal
convergence bound. The backend therefore supplies an actual completed local
reasoning result to the driver. No arbitrary fixed-point timeout is assumed.

Replaying the lower rules preserves information absent from surviving rows:

```text
stage 0:  => p       => not p      p is ambiguous: lambda p, but no +d p
stage 1:  => q       p ~> not q    lambda p keeps the attack on q applicable
```

With no priorities, the completed prefix cannot defeasibly prove `q`. Replacing
stage zero with positive conclusions alone drops the potential support for `p`
and incorrectly permits `q`. `GroundBackendTests.lean` checks both outcomes.
Thus `carry_fact_iff` remains a proof about emitted rule types, not a theorem
that positive-only transport preserves reasoning. Folds still select surviving
positive rows; ordinary attack analysis needs richer information.

The backend checks ordinary body-to-head ordering, shared ownership of
complements, and empty fact bodies. `groundScheduled_iff` proves that its Boolean
check is equivalent to these conditions. It retains labels and priorities rather
than synthesizing positive-only support rules. Prefix replay is a reference
strategy, not an optimization. Publication uses the full theory's finite literal
set, so body-only lower literals receive their negative tags at their own stage.

## Schema lowering and equivalence

`evaluateProgram policy program domain` connects inference, finite grounding,
aggregate lowering, and the checked stage driver. The result includes the final
ground theory and the accumulated tagged conclusions for independent replay.

```text
typed schemas + finite domain
          |
          v
infer stages --> ground stage 0 --> reason complete prefix
                                         |
                                         v
                              surviving distinct ground rows
                                         |
                                         v
                         evaluate folds; lower next stage
                                         |
                                         v
                         final ground theory + scheduled run
```

The supported executable fragment has static, nonmodal predicate patterns,
multiple heads, strong negation, all ordinary rule kinds, integer expressions,
binds/comparisons, and sum/min/max folds. Count uses constant extraction of one.
The domain must contain aggregate result values as well as input values; a result
outside it is an error. Unknown functions/reducers, nonground domains, malformed
facts, aggregate cycles, and unsupported modes are rejected. This uses exact
integer arithmetic, not machine overflow or floating-point behavior.

The evidence policy is an explicit argument. `defeasibleEvidence` adds a fresh
unconditional defeasible closure guard to an aggregate-bearing rule's premises;
the original rule kind is preserved. Even a strict rule therefore cannot obtain
a definite proof through this aggregate premise. `rejectStrict` instead rejects
aggregate-bearing strict rules. Guard atoms cannot collide with encoded user
atoms. Original schema priorities apply to their grounded instances; inserted
guards use a separate label namespace.

The proofs distinguish three claims:

1. Scheduled ordinary prefixes agree with full-theory delta, lambda, and partial
   closures in every completed domain. Exact equality of the published batches
   includes all four proof tags, including negative tags for body-only literals.
2. Successful later lowering preserves earlier rules and priorities, and therefore
   the completed closures that earlier folds read (`lowerStages_completed_agree`).
3. Successful `evaluateProgram` execution has exactly the same tagged conclusions
   as running `reason` once on the final lowered theory (`evaluateProgram_correct`).
   This is equality of conclusion membership; stage ordering can change list order.

The comparison theory in the last claim is the theory produced by the declared
lowering policy. The theorem does not assign a universal semantics to every
possible aggregate extension or prove conformance of the Rust implementation.

## Remaining work

1. Connect actual SPL/Rust parsing to this typed schema fragment and prove the
   translation preserves constructs. Extend executable support for modes,
   temporal patterns, schematic predicates, and broader expression types.
2. Extend the finite-domain grounding contract to generated values, with a
   termination or explicit resource-limit contract. Prove broader declarative
   grounding completeness for the aggregate language; the current proof starts
   from successful execution of the defined finite lowerer.
3. Specify machine arithmetic, decimals, extension values, and richer failures.
   Exact integer addition is lawful; checked or floating-point addition cannot
   inherit the same reducer laws without a refinement proof.
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
