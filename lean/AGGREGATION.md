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
| `Spindle/Aggregation/GroundBackend.lean` | Re-runs each retained finite ground rule prefix through the traditional four-tag reasoner. `groundBackend_completed` proves each batch comes from the completed four-tag closure; `groundBackend_runs` proves recognized inputs finish every requested finite stage count. |
| `Spindle/Aggregation/GroundBackendTests.lean` | Kernel-checked regressions cover ambiguous lower support, attack reachability, original priorities, proof strength, ordinary recursion, defeated aggregate rows, and rejected schedules. |
| `Spindle/Aggregation/PrefixEquivalence.lean` | `reason_agrees_on_closed_domains` proves agreement of all four constructive tags on dependency-closed domains. `reasonedStage_equivalent` proves exact tagged-batch equality; `executeGround_membership` covers accumulated execution. |
| `Spindle/Aggregation/Lowering.lean` | Grounds typed schemas over a finite domain and lowers folds stage by stage. Atom encoding is injective on the table and cannot collide with closure guards. `lowerStages_completed_agree` preserves completed reasoning during lowering. `evaluateProgram_correct` proves scheduled execution has exactly the full lowered theory's tagged conclusions. |
| `Spindle/Aggregation/LoweringTests.lean` | Runtime integration assertions cover inference, grounding, grouping, duplicate rows, defeated rows, priorities, multiple heads, chained folds, evidence policies, empty inputs, and explicit scope/domain errors. |
| `Spindle/Aggregation/Primitives.lean` | Shared variable scoping, integer expressions, and row matching; no aggregate evaluation or rule lowering. |
| `Spindle/Aggregation/SourceSemantics.lean` | Independent relational aggregate satisfaction over any enumeration of the row set and any reduction order. Includes seeds, required emptiness, finite assignments, static conditions, and structured source clauses. `Fold.rows_extensional` proves that row order and duplicate proofs are irrelevant. |
| `Spindle/Aggregation/LoweringCorrectness.lean` | `evalSchemaFold_iff` proves aggregate evaluation sound and complete. `lowerInstance_correct` and `lowerBatch_correct` characterize exactly the emitted rules; `lowerInstance_complete` proves permitted, defined, covered source instances successfully lower. `lowerProgram_source_correct` gives a source-stage derivation for the final theory; `evaluateProgram_source_correct` composes this with tagged execution equivalence. |
| `Spindle/Aggregation/LoweringCorrectnessTests.lean` | Kernel-checked source derivations cover duplicate rows, equal contributions, reordered enumeration, a non-identity seed, empty folds, domain exclusion, and a strict multi-head instance retaining an ordinary premise. |

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
`Operational.reason` over this prefix and publishes only the current stage's
conclusions. All four tags are derived constructively; no closure complement
is reported as a negative proof.
`Operational.close_fixedpoint` proves completion of the joint constructive
positive/negative closure. Each round adds fresh tagged literals, so the finite
bound is sufficient; no arbitrary fixed-point timeout is assumed.

Replaying the lower rules preserves the support and attack structure, including
constructive negative evidence:

```text
stage 0:  => p       => not p      conflicting support establishes -d p
stage 1:  => q       p ~> not q    -d p discards the attacker; +d q
```

With no priorities, both Lean and Rust now prove `q`. Lambda membership alone
does not keep a defeated premise alive. `GroundBackendTests.lean` checks this
behavior. Folds select surviving positive rows from the completed result.
Ordinary reasoning retains the original rules and priorities, rather than
reconstructing a theory from positive conclusions alone.

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

The selected default is defeasible snapshot evidence. Both `lowerProgram` and
`evaluateProgram` default their policy argument to `defeasibleEvidence`; for
example, `evaluateProgram (program := program) (domain := domain)`. This adds a fresh
unconditional defeasible closure guard to an aggregate-bearing rule's premises;
the original rule kind is preserved. Even a strict rule therefore cannot obtain
a definite proof through this aggregate premise. An independent ordinary proof
may still establish `+D` for the same head. `rejectStrict` remains an explicit
opt-in restriction that rejects aggregate-bearing strict rules. Guard atoms cannot collide with encoded user
atoms. Original schema priorities apply to their grounded instances; inserted
guards use a separate label namespace.

The proofs distinguish three claims:

1. Scheduled ordinary prefixes agree with all four full-theory constructive
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

## Independent source semantics and lowering correctness

`SourceSemantics.lean` does not import the lowerer. Its `Source.Fold` judgment
selects a duplicate-free enumeration with exactly the input relation's members,
requires one extraction for every matching row, and derives the result by an
unordered reduction relation. It does not call `evalSchemaFold`, `Reducer.eval`,
or any rule-emission function. The source and compiler share the language's
integer-expression evaluator, row matcher, and variable-scope definitions. This
proof verifies aggregation and lowering relative to those primitives; it does
not independently reverify expression arithmetic or parsing.

```text
source row set + outer assignment
              |
              v
 one contribution per matching row
              |
              v
 unordered reduction + seed + domain bound
              |
              v
 enabled source clause, with ordinary premises retained
              |
              v
 encoded rule(s) + explicit defeasible closure witness
```

`evalSchemaFold_iff` is a two-way equivalence, including empty required folds:
`evalSchemaFold ... = ok result` iff `Source.Fold ... result`. An unknown reducer,
failed extraction, or out-of-domain result has no successful source judgment.
`sourceFold_deterministic` proves that the freedom in enumeration and reduction
order cannot produce different results. Distinct rows with equal values still
supply separate contributions; a seed need not be an identity and occurs once.

`lowerInstance_correct` characterizes rule membership after successful lowering:
a rule is emitted iff it encodes an enabled source clause or is that instance's
required closure witness. All heads and the original rule kind are retained.
An ordinary premise is residual: compiling a rule does not assert that its body
is already proved. `lowerInstance_complete` additionally proves successful
lowering from source satisfaction, provided the evidence policy permits the
instance and its ground atoms are covered by the encoding table. This separate
result does not assume compiler success.

`assignment_iff` relates the executable Cartesian product to the source's finite
assignment judgment. `lowerBatch_correct` lifts the instance equivalence to
all assignments of all schemas in a stage. `SourceStages` specifies a sequence
of theories, each extending its predecessor by exactly the source-justified
rules read against that predecessor's completed SDL reasoning. It preserves
priorities and does not invoke the lowering functions.

`lowerProgram_source_correct` proves every successful program lowering has this
source-stage derivation. `evaluateProgram_source_correct` combines that result
with equality of all four tagged conclusion memberships against full replay.
Program acceptance remains a premise: this is not a claim that every syntactically
supported program fits a particular finite domain or passes all grounding checks.
The existing inference and prefix theorems supply the completed-stratum ordering
and preservation guarantees described above.

## Rust differential bridge

`spindle_core::aggregation::evaluate` accepts typed nonmodal schemas and an
explicit finite domain. It implements its own grounding, stage inference, fold
evaluation, and rule lowering, and calls the existing Rust reasoner on each
completed prefix. Unique ground labels prevent Rust's label-indexed theory from
overwriting instances; source superiority pairs expand across all corresponding
instances. The selected policy is defeasible snapshot evidence.

`AggregationOracle` parses the same typed schema wire format and invokes the
verified Lean `evaluateProgram` entry point. It returns the inferred stage count
and all four tagged conclusions decoded to structured user atoms. Internal guards
are omitted. Both sides expose only atoms mentioned in their own lowered theory;
Rust's extra synthesized complement negatives are outside this comparison.
Neither side consumes the other side's lowered rules or expected fold values.

Run the bridge explicitly (the external-oracle tests are ignored by ordinary
Cargo runs and mandatory in the Lean CI step):

```sh
(cd lean && lake build AggregationOracle)
cargo test -p spindle-core --test lean_aggregation_oracle_difftest -- --ignored --nocapture
```

Missing binaries, truncated batches, malformed responses, and unexpected
mismatches fail the tests. Deterministic generated cases cover small integer
relations, seeds, reducers, and evidence strength; curated cases cover grouping,
multiple heads, defeated rows, source priorities, chained folds, residual logic,
binds, comparisons, rejection, and duplicate proofs. Rust-only regressions run
without Lean and check the snapshot policy and overflow rejection.

The former ordinary-backend counterexample is now a mandatory agreement case:

```text
=> p       => not p       p ~> not q       => q
                                      |
                                      v
                              total = count(q)

Lean and Rust:                    +d q, total(1)
```

`constructive_discard_agrees_with_lean_aggregate_snapshot` requires equality
of all reported tags and explicitly checks the surviving row and count.
`generated_constructive_conflicts_agree` adds 512 combinations of supports,
cycles, attackers, and priorities. No expected-disagreement test remains.
The aggregate completion, closed-domain prefix, and independent source-lowering
proofs now use `Operational.reason`, the traditional nonmodal four-tag inference conditions. The older lambda-only reasoner remains available for its
own core proofs, but is no longer the aggregate oracle's backend.

### Which ordinary semantics?

Traditional DL(∂) discards an attacker when a premise is constructively disproved.
DL(∂∥) is a different published logic: it tests absence from the potential-support
closure λ instead. Applying those conditions to the example above gives one
surviving q row in DL(∂), and none in DL(∂∥). See
[Maher et al., sections 2 and 4](https://doras.dcu.ie/24726/1/Scalable_Defeasible_Logic.pdf).

The Rust default and aggregate Lean model now use traditional DL(∂): negative
tags must be constructively derived, unseeded cycles remain undecided, and
+D unconditionally implies +d. The prior lambda-based negative seeding,
strict-inconsistency gate, and complement-based negative reports have been removed.
See the [formal semantics](../specs/DEFEASIBLE-LOGIC-SEMANTICS.md) for the four
conditions and compatibility examples. The proofs establish completion and
lowering for these conditions; the differential tests check the Rust implementation.

Additional scope limits: the Rust API uses checked i64 arithmetic and rejects
overflow, while the aggregate Lean model uses exact Int. Agreement generation
stays within safe arithmetic bounds; no machine-arithmetic refinement is claimed.
The JSON adapter accepts integer/symbol/variable terms and expressions nested at
most 64 levels. The SPL bridge now parses folds and enforces an enumeration budget; these Rust
integrations are tested, not proved. Decimals, modes, and temporal patterns remain
unsupported in aggregate programs. Error acceptance is compared; error
message wording is not required to match.

## SPL and extension integration

The normal `prepare` path detects explicit `ArithExpr::Fold` nodes, translates
source rules with `aggregation::source::program`, evaluates completed prefixes,
and decodes the retained rules back to structured literals. It preserves source
labels as `template_label`, metadata, predicate declarations, and expanded
priorities. Fresh snapshot predicates are collision-checked and hidden from
reasoning conclusions. Indexing an unprepared fold returns an error.

The [syntax guide](../docs/aggregation-extensions.md) documents the fragment and
limits. `spl_pipeline_agrees_with_lean_aggregate_oracle` exercises 26 parsed
programs, including grouping, empty inputs, builtin extraction, and chained
strata. CLI tests exercise reasoning and querying through the same preparation.

Builtins and custom functions use Claire's registry architecture from PRs #26
and #27. General symbol-valued bindings and arbitrary extension calls extend
beyond the verified integer-expression fragment. The typed `Condition::Bind`
retains its original integer/error semantics; the source bridge uses `BindValue`
for general bindings. An extension's Rust implementation is trusted code, not a
proof of purity or of its correspondence to a Lean function. Fold reducers remain
engine-controlled sum/min/max; registry overrides do not redefine them.

## Remaining work

1. Prove the implemented SPL-to-schema translation preserves constructs. Extend executable support for modes,
   temporal patterns, schematic predicates, and broader expression types.
2. Prove the Rust predicate grounder complete. It now binds computed values and
   uses a grounding work budget, but the whole-program lowering proofs still
   cover the finite reference model. The new domain-free aggregate operation
   has its own soundness/completeness proof; this is not a proof of the grounder.
3. Specify machine arithmetic, decimals, extension values, and richer failures.
   Exact integer addition is lawful; checked or floating-point addition cannot
   inherit the same reducer laws without a refinement proof.
4. Extend the implemented CLI/SPL differential coverage to temporal filtering
   and richer provenance. A formal refinement of the Rust implementation remains
   separate from the Lean proofs and executable differential evidence.

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

## Predicate aggregate bindings

SPL also accepts `(agg ?total sum ?amount (payment ?id ?amount))` as a direct
rule premise. `sum`, `count`, `min-of`, and `max-of` resolve through the registry
to the verified fold kernel with their own empty-input policies. The parsed
builtin cases are differentially tested against this Lean model. Host-provided
combining functions are an unproved extension. SPL reads ordinary predicates,
including derived rows, and binds new totals without an `aggregate-domain`.
Multi-condition aggregate subqueries still require a helper predicate.

`Source.PredicateFold` removes the finite result-membership condition from the
independent unordered source judgment. `evalPredicateFold_iff` (in the
`Spindle.Aggregation` namespace) proves the executable domain-free fold sound and
complete. `predicateFold_finite_iff` connects it to the existing finite model.
Fresh output insertion and existing-output equality constraints are proved too.

The Rust grounder joins potential predicate instances and conservatively retains
ordinary cycles over source/computed constants. It repeats completed stages as
that set grows and fails when its work budget is exhausted. This grounder is
**tested, not proved**. Differential tests compare every positive conclusion and
all four tags on retained atoms; the exhaustive finite oracle can additionally
mention impossible ground instances with only negative tags. These extra atoms
are checked to have no positive conclusions, rather than requiring the sparse
SPL grounder to materialize them. See [the syntax guide](../docs/aggregation-extensions.md).
