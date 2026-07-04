# Spindle Lean 4 Formal Verification

This directory contains a complete formal verification of the Spindle defeasible logic engine in Lean 4.27.0 with Mathlib 4.27.0.

**Status:** 0 sorry, 0 custom axioms, 300+ proven theorems.

**Verification guarantees** (as of 2026-07-04):

- The root module `Spindle.lean` imports every `Spindle/Arith/*` module, so
  `lake build Spindle SpindleLean` type-checks the whole library — no orphaned
  modules whose proofs silently escape compilation.
- `AxiomAudit.lean` runs `#print axioms` over the flagship theorems. Every one
  depends only on Lean's three standard axioms (`propext`, `Classical.choice`,
  `Quot.sound`) — several on fewer. In particular there is no `sorryAx` (no
  `sorry` anywhere in a dependency) and no `Lean.ofReduceBool` (no
  `native_decide`; nothing trusts the compiler). Re-run with:

  ```sh
  cd lean && lake build Spindle SpindleLean && lake env lean AxiomAudit.lean
  ```

## What Is Proven

### Core Properties of DL(d) Reasoning

These proofs establish that the three-phase closure algorithm (delta → lambda → partial) correctly implements the DL(d) defeasible logic framework from Billington, Antoniou, Governatori, and Maher.

#### Soundness (`Properties/Soundness.lean`)

Every conclusion the engine produces is justified by the theory's rules.

| Theorem | Statement |
|---------|-----------|
| `delta_sound` | Every literal in delta(T) is the head of a definite rule in T |
| `canProve_false_of_undefeated_attacker` | If an undefeated attacker exists, `canProve` returns false |
| `ambiguity_blocks_both` | When competing rules have no superiority resolution, neither literal is defeasibly provable |

#### Subset Chain (`Properties/Subset.lean`)

The three closure sets satisfy the inclusion chain delta ⊆ partial ⊆ lambda.

| Theorem | Statement |
|---------|-----------|
| `delta_subset_partial` | Every definitely provable literal is defeasibly provable |
| `delta_subset_lambda` | Every definitely provable literal is in the lambda over-approximation |
| `partial_subset_lambda` | Every defeasibly provable literal is in the lambda over-approximation |
| `mem_deltaStep_of_mem` | Step functions never remove elements (extensiveness) |

#### Termination (`Properties/Termination.lean`)

All closure computations terminate on finite theories.

| Theorem | Statement |
|---------|-----------|
| `fixpoint_stable_delta` | When the length check detects a fixpoint, deltaStep is the identity |
| `deltaClose_converges_bound` | Delta closure converges within \|allLiterals\| iterations |
| `lambdaClose_converges_bound` | Lambda closure converges within \|allLiterals\| iterations |
| `partialClose_converges_bound` | Partial closure converges within \|allLiterals\| iterations |
| `deltaClose_fuel_independent` | Any sufficient fuel value produces the same result |

**Proof technique:** Pigeonhole argument — each step either adds a new literal from a finite universe or detects the fixpoint.

#### Confluence (`Properties/Confluence.lean`)

The fixpoint is unique and independent of evaluation order.

| Theorem | Statement |
|---------|-----------|
| `bodySatisfied_mono` | Body satisfaction is monotone over list subsets |
| `deltaStep_mono` | deltaStep is monotone: S₁ ⊆ S₂ implies step(S₁) ⊆ step(S₂) |
| `lambdaStep_mono` | lambdaStep is monotone |
| `delta_confluence` | Delta closure converges to a unique fixed point |
| `lambda_confluence` | Lambda closure converges to a unique fixed point |
| `partial_confluence` | Partial closure converges to a unique fixed point |
| `reason_deterministic` | The full reasoning pipeline is deterministic |

**Proof technique:** Monotonicity + extensiveness + finite universe → Knaster-Tarski guarantees a unique least fixed point.

#### Equivalence (`Properties/Equivalence.lean`)

The implementation computes exactly the DL(d) consequence operator.

| Theorem | Statement |
|---------|-----------|
| `reason_plusD_sound` | If the engine concludes +D l, then l has a definite derivation |
| `reason_plusD_complete` | If l has a definite derivation, then the engine concludes +D l |
| `three_phase_subset_chain` | (delta-consistent) delta ⊆ partial ⊆ lambda for the three-phase decomposition |

#### Faithfulness (`Properties/Faithfulness.lean`)

The implementation matches the spec's formal definitions of +D and +d
(`specs/DEFEASIBLE-LOGIC-SEMANTICS.md`). Note the spec's +d deliberately
deviates from Antoniou et al.: condition (2) (`-D ~q`) gates the
delta-subsumption clause too, quarantining strict contradictions.

| Theorem | Statement |
|---------|-----------|
| `faithful_plusD_forward` | l ∈ delta(T) implies l satisfies the paper's +D condition |
| `faithful_plusD_backward` | The paper's +D condition implies l ∈ delta(T) |
| `faithful_plusd_forward` | l ∈ partial(T) implies l satisfies the spec's (gated) +d condition |
| `faithful_plusd_backward` | The spec's +d condition implies l ∈ partial(T) |
| `faithful_ambiguity_blocking` | Ambiguity blocking matches the paper's semantics |
| `faithful_D_implies_d` | +D l → +d l whenever ~l is not also +D (delta-consistency hypothesis) |

#### Consistency (`Properties/Consistency.lean`)

The payoff of the gated subsumption: the defeasible level is consistent
for **every** input — a theorem standard DL does not have (there, an
inconsistent strict part propagates +∂p and +∂~p).

| Theorem | Statement |
|---------|-----------|
| `partial_consistent` | For well-formed theories with well-founded superiority, partial(T) never contains both l and ~l |
| `partial_consistent_no_superiority` | Specialization: consistency for theories with no superiority declarations |
| `supRel_wellFounded_of_no_superiority` | Empty superiority is trivially well-founded |

Proof technique: well-founded induction on the superiority relation
replaces the classic infinite-descent argument — each side's supporter is
an applicable attacker of the other side, and team defeat forces a
strictly superior applicable defender, ascending forever unless the
relation is well-founded. Superiority cycles genuinely break consistency
(two mutually-superior opposing rules both win), so the hypothesis is
necessary.

#### Acyclicity (`Properties/Acyclicity.lean`)

The superiority relation remains well-formed under theory construction.

| Theorem | Statement |
|---------|-----------|
| `empty_acyclic` | The empty theory has an acyclic superiority relation |
| `empty_irreflexive` | The empty theory has an irreflexive superiority relation |
| `addSuperiority_preserves_acyclic` | Adding a non-cyclic, non-self-loop superiority pair preserves acyclicity |

#### Temporal-Family Reasoning (`SpindleLean/Family.lean`)

The SPEC-020 semantics for ground temporal theories, established by
direct engine probing (`crates/spindle-core/tests/family_probe.rs`):
exact-identity conflict (the complement keeps its window; disjoint,
overlapping, and atemporal complements never conflict) and family
support for atemporal bodies only (temporal bodies need exact windows),
applied uniformly across the definite phase, defeasible phase, and
defeater bodies.

| Theorem | Statement |
|---------|-----------|
| `FLit.famSat_iff` | Family-support characterization: satisfied iff exactly proven, or atemporal with a proven family member |
| `FLit.famSat_eq_exactSat_of_atemporal` | On window-free sets, family satisfaction collapses to exact membership |
| `bodySat_eq_exact_of_atemporal`, `deltaStepWith_parity` | **Non-temporal parity**: the family machinery is invisible on window-free theories (SPEC-020's non-temporal equivalence requirement) |

Difftested exhaustively (`lean_family_exhaustive_difftest.rs`): 400,730
ground temporal theories at full scope, zero divergences — after fixing
an order-dependent family-discard bug in the engine that the difftest
surfaced (see `lean/DIVERGENCES.md`, class 3).

#### As-Of Temporal Filter (`Spindle/Arith/AsOfFilter.lean`)

Formalizes `filter_temporal` (the `PrepareOptions.reference_time`
pipeline stage).

| Theorem | Statement |
|---------|-----------|
| `mem_filterTemporal` | The as-of contract: a rule survives iff active at the reference time (rule window AND head AND body windows) |
| `filterTemporal_subset`, `filterTemporal_idempotent` | Filtering only removes; same-instant filtering is idempotent |
| `filterTemporal_atemporal` | Window-free theories pass through unchanged |
| `filterSup_kept` | A superiority pair survives iff both endpoint rules survive |

#### Requires Operator (`Spindle/Arith/Requires.lean`)

Formalizes the verified `requires` search (IMPL-011): abduce candidates
verified by injection and re-reasoning.

| Theorem | Statement |
|---------|-----------|
| `requiresVerify_facts_mem` | Acceptance contract: a candidate is returned iff injecting it makes the goal provable |
| `requiresVerify_sound`, `requiresVerify_rejected` | Accepted candidates derive the goal; rejected ones genuinely fail |
| `requires_whatIf`, `requires_refines_abduce` | Cross-operator consistency with what-if and abduce |

#### SPL Grammar Fragment (`Spindle/Spl/Grammar.lean`)

The propositional-SDL fragment of the SPL input grammar (LangSec: the
grammar is part of the contract), with canonical printer and parser.

| Theorem | Statement |
|---------|-----------|
| `decode_encode_stmt`, `decode_encode_theory` | **Grammar roundtrip**: canonically encoded theories decode losslessly — no fragment statement is ambiguous or unrepresentable |

The string layer is exercised end-to-end by `spl_parser_difftest.rs`:
1,684 fragment theories, canonical text parsed identically by the Lean
model and `spindle-parser`.

---

### Trust Layer (`Spindle/Trust/`)

The trust-weighted reasoning layer (`crates/spindle-core/src/trust.rs`) is
formalized over exact rationals. The model mirrors the f64 implementation's
computations exactly (clamped subtraction, running-minimum fold, clamp-based
decay) and is difftested against it (`lean_trust_oracle_difftest.rs`).

#### Diminishment — graduated defeat (`Trust/Diminish.lean`)

| Theorem | Statement |
|---------|-----------|
| `diminish_eq_mul` | The implementation's clamped-subtraction form equals the paper's multiplicative operator τ_c(1−τ_d) on the unit interval |
| `diminish_le_self` | Pollock constraint 1 / bounded reduction: J(c,d) ≤ c |
| `diminish_zero` | Pollock constraint 2 / zero-defeat limit: J(c,0) = c |
| `diminish_one` | Full-defeat limit: J(c,1) = 0 (recovers binary defeat) |
| `diminish_antitone` | Monotonicity: a more credible defeater reduces more |
| `diminish_pos` | Relaxed third constraint: any d < 1 leaves a positive residue — full defeat only at d = 1 |
| `diminish_diminish_comm` | Two diminishers commute |
| `diminishAll_eq_prod` | n diminishers yield the product form c·∏(1−dᵢ) (order-independence) |
| `diminishAll_le_single` | Collective diminishment is at least as strong as any individual diminisher |

#### Weakest-link propagation (`Trust/WeakestLink.lean`)

| Theorem | Statement |
|---------|-----------|
| `weakestLink_le_trust` | A conclusion is never more trusted than the rule that derived it |
| `weakestLink_le_child` | A conclusion is never more trusted than any premise it rests on |
| `le_weakestLink` | Weakest link is the greatest such lower bound |
| `weakestLink_mem_unit` | Chain degrees stay in [0,1] |
| `diminishAll_weakestLink_le_child` | Composition: the diminished chain degree is still bounded by every link |

#### Temporal decay (`Trust/Decay.lean`)

| Theorem | Statement |
|---------|-----------|
| `linearDecay_antitone`, `stepDecay_antitone` | Trust never recovers with age |
| `linearDecay_nonneg/le_one`, `stepDecay_nonneg/le_one` | Multipliers stay in [0,1] |
| `linearDecay_at_zero`, `stepDecay_at_zero` | Fresh testimony has full trust |
| `effectiveTrust_le_base` | Decay never increases trust |
| `DecayLaw.effective_mem_unit` | Any decay law keeps effective trust in [0, base] |

`DecayLaw` is the abstract interface (range, freshness, antitonicity);
linear and step decay are proven instances. Exponential decay
(0.5^(age/half-life)) is irrational, satisfies the same interface over ℝ,
and is exercised by the Rust unit tests; it is excluded from the
rational-exact oracle.

---

### Arithmetic and Grounding

These proofs verify the arithmetic subsystem: type promotion, expression evaluation, grounding, and the Herbrand base construction.

#### Type Promotion (`Arith/Promotion.lean`)

The numeric type lattice (int ≤ decimal) preserves values under promotion.

| Theorem | Statement |
|---------|-----------|
| `lub_comm`, `lub_idem`, `lub_assoc` | Least upper bound forms a lattice |
| `promote_self` | Promoting a value to its own type is the identity |
| `promote_int_decimal` | int(n) promotes to decimal(n, 0) |
| `numeric_eq_refl` | Numeric equality is reflexive |
| `numeric_eq_symm` | Numeric equality is symmetric |

#### Expression Evaluation (`Arith/Eval.lean`, `Arith/Constraint.lean`)

Arithmetic expressions and constraints evaluate deterministically.

| Theorem | Statement |
|---------|-----------|
| `ArithExpr.eval_deterministic` | Expression evaluation is a function (same inputs → same output) |
| `ArithConstraint.eval_deterministic` | Constraint evaluation is deterministic |
| `ArithExpr.eval_var_of_bound` | Variables evaluate when bound in the environment |
| `evalConstraints_nil` | Empty constraint list always succeeds |

#### Grounding (`Arith/GroundLiteral.lean`, `Arith/GroundRule.lean`, `Arith/GroundingCompleteness.lean`)

Variable instantiation is sound and complete.

| Theorem | Statement |
|---------|-----------|
| `Literal.applySubst_ground` | Applying a ground substitution produces a ground literal |
| `Rule.applySubst_ground` | Applying a ground substitution produces a ground rule |
| `Rule.groundInstances_complete` | Enumeration finds all ground instances of a rule |
| `allSubstitutions_complete` | The substitution enumeration covers all possibilities |

#### Herbrand Base (`Arith/HerbrandBase.lean`)

The universe of ground atoms is finite and correctly bounded.

| Theorem | Statement |
|---------|-----------|
| `herbrandBase_ground` | All generated atoms are ground |
| `herbrandBase_count` | Exact count: Σ \|dom\|^arityᵢ |
| `herbrandBase_finite` | Size bounded by \|sigs\| × \|dom\|^maxArity |

#### Pattern Matching (`Arith/Matching.lean`)

Unification produces correct substitutions.

| Theorem | Statement |
|---------|-----------|
| `matchTerm_sound` | Matching substitution applies the pattern to the target |
| `matchTerms_sound` | Multi-term matching is sound |
| `matchLiteral_sound` | Literal matching produces correct substitutions |
| `matchLiteral_ground` | Ground matching of ground literals succeeds |

#### Semi-Naive Evaluation (`Arith/SemiNaive.lean`)

The immediate consequence operator T_P is monotone and cumulative.

| Theorem | Statement |
|---------|-----------|
| `T_P_mono` | T_P is monotone in its rule set |
| `Rule.firesb_mono` | Rule firing is monotone in the interpretation |
| `T_P_iter_mono` | Iteration is monotone |
| `T_P_iter_cumulative` | Iterations accumulate (T_P^n ⊆ T_P^(n+1)) |

---

### Temporal Reasoning

Proofs about Allen's interval algebra and temporal constraint evaluation.

#### Allen Relations (`Arith/AllenRelation.lean`, `Arith/Composition.lean`)

| Theorem | Statement |
|---------|-----------|
| `AllenRelation.inverse_involution` | inverse(inverse(r)) = r |
| `AllenRelation.classify_holds` | Classification matches Allen's 13 interval relations |
| `AllenRelation.holds_unique` | Each interval pair has exactly one Allen relation |
| `AllenRelation.compose_sound` | Composition respects transitivity |

#### Interval Sets (`Arith/IntervalSet.lean`)

| Theorem | Statement |
|---------|-----------|
| `IntervalSet.normalize_covers` | Normalization preserves point-set semantics |
| `IntervalSet.normalize_idempotent_covers` | Normalizing twice = normalizing once |
| `IntervalSet.intersection_comm` | Intersection is commutative |
| `IntervalSet.intersection_spec` | A point is in A ∩ B iff it is in both A and B |
| `IntervalSet.subtraction_spec` | Subtraction removes exactly the blocked points |

#### Temporal Grounding (`Arith/TemporalGrounding.lean`)

| Theorem | Statement |
|---------|-----------|
| `evaluateConstraint_deterministic` | Temporal constraint evaluation is deterministic |
| `evaluateConstraint_holds_of_satisfied` | Satisfied constraints imply the Allen relation holds |
| `evaluateConstraint_satisfied_witness` | Satisfied constraints have witnessing intervals |

---

### Query Operators

Proofs about the three query operators: what-if, why-not, and abduction.

#### What-If (`Arith/WhatIf.lean`)

| Theorem | Statement |
|---------|-----------|
| `whatIf_empty` | what_if(T, ∅) produces no new conclusions |
| `whatIf_mono` | what_if is monotone in hypothetical facts |
| `whatIf_derivable` | what_if conclusions derive from T ∪ F |
| `whatIf_not_baseline` | what_if conclusions are genuinely new |

#### Why-Not (`Arith/WhyNot.lean`)

| Theorem | Statement |
|---------|-----------|
| `whyNot_missingPremise_not_derived` | Missing premises are genuinely not derived |
| `whyNot_missingPremise_in_body` | Missing premises come from rule bodies |
| `whyNot_contradicted_derived` | Contradicting literals are actually derived |
| `whyNot_chain` | Missing premises form a well-founded chain |

#### Abduction (`Arith/Abduce.lean`)

| Theorem | Statement |
|---------|-----------|
| `abduce_solution_valid` | Every abduction solution is valid |
| `abduce_implies_whatIf` | Abduction solutions are what-if conclusions |
| `whyNot_guides_abduce` | Why-not results guide abduction search |
| `abduce_trivial_minimal` | The empty solution is minimal |

#### Cross-Operator Soundness (`Arith/QuerySoundness.lean`)

| Theorem | Statement |
|---------|-----------|
| `abduce_whatIf_soundness` | Abduction and what-if are consistent |
| `whatIf_abduce_roundtrip` | Round-trip: abduce then what-if recovers the goal |
| `whyNot_whatIf_bridge` | Why-not missing premises are what-if candidates |
| `pipeline_soundness` | The full query pipeline satisfies all soundness properties |

---

## Architecture

```
SpindleLean/
├── Basic.lean          # Literal, Mode types + complement involution
├── Rule.lean           # Rule, RuleType + bodySatisfied
├── Theory.lean         # Theory type + WellFormed predicate
├── Reason.lean         # Three-phase closure (delta/lambda/partial) + reason()
└── Properties/
    ├── Soundness.lean      # Every conclusion is justified
    ├── Subset.lean         # delta ⊆ partial ⊆ lambda
    ├── Termination.lean    # All closures terminate
    ├── Confluence.lean     # Unique fixed point
    ├── Equivalence.lean    # Implementation = DL(d) semantics
    ├── Faithfulness.lean   # Matches paper definitions
    └── Acyclicity.lean     # Superiority well-formedness

Spindle/Arith/
├── Types.lean              # Value (int, decimal), operators
├── Promotion.lean          # Type lattice + numeric equality
├── Eval.lean               # Arithmetic evaluation
├── Constraint.lean         # Constraint evaluation
├── Term.lean               # First-order terms
├── Substitution.lean       # Substitutions + application
├── Matching.lean           # Pattern matching / unification
├── GroundLiteral.lean      # Ground literals
├── GroundRule.lean          # Ground rules + instantiation
├── GroundingCompleteness.lean  # Grounding is complete
├── GroundingCompat.lean    # Grounding compatibility
├── HerbrandBase.lean       # Finite universe of ground atoms
├── SemiNaive.lean          # T_P operator + monotonicity
├── TimePoint.lean          # Temporal points (negInf, moment, posInf)
├── Interval.lean           # Closed intervals
├── IntervalSet.lean        # Interval set operations
├── AllenRelation.lean      # Allen's 13 interval relations
├── Composition.lean        # Allen relation composition
├── TemporalGrounding.lean  # Temporal constraint evaluation
├── WhatIf.lean             # Hypothetical reasoning
├── WhyNot.lean             # Explanation / debugging
├── Abduce.lean             # Abductive reasoning
└── QuerySoundness.lean     # Cross-operator consistency

Spindle/DiffTest/
├── ArithOracle.lean        # Arithmetic differential testing oracle
├── EndToEndOracle.lean     # End-to-end oracle
└── GroundingOracle.lean    # Grounding oracle
```

## Assumptions

The proofs make the following explicit assumptions (as theorem hypotheses, not axioms):

- **`Theory.WellFormed`**: Fact rules have empty bodies (enforced by the `Rule.fact` constructor in practice).
- **`t.allLiterals.length ≤ 1000`**: Theories have at most 1000 distinct literals (the hardcoded fuel bound). This is a practical bound, not a fundamental limitation.
- **`w ≠ l`** in `addSuperiority_preserves_acyclic`: Self-loops are excluded (they trivially create cycles).
- **`0 < dom.length`** in `herbrandBase_finite`: The domain is nonempty (empty domains make the bound vacuously wrong due to 0^0 = 1).

## Axioms

Only standard Lean 4 axioms are used:

- `propext` (propositional extensionality)
- `Classical.choice` (classical logic)
- `Quot.sound` (quotient soundness)
- `Lean.ofReduceBool` / `Lean.trustCompiler` (kernel reduction trust, from `simp`/`decide`)

No custom axioms.

## Differential Testing (Lean vs Rust)

Three proptest-based test suites verify that the Lean formalization agrees with the Rust implementation on concrete inputs. Each test generates random inputs, evaluates them in both Rust and Lean (via JSON over stdin/stdout), and asserts the results match.

| Test suite | What it compares | Cases |
|-----------|-----------------|-------|
| `lean_arith_oracle_difftest.rs` | Rust `arith.rs` vs Lean `Eval.lean` | Random arithmetic expressions |
| `lean_grounding_oracle_difftest.rs` | Rust `grounding.rs` vs Lean `GroundRule.lean` | Random rules + domains → ground instances |
| `lean_end_to_end_difftest.rs` | Full Rust pipeline vs full Lean pipeline | Non-ground theories → ground + reason → +D conclusions |

Run with:
```bash
cd lean && lake build                    # Build Lean oracles
cd .. && cargo test --test lean_arith_oracle_difftest -- --ignored
cargo test --test lean_grounding_oracle_difftest -- --ignored
cargo test --test lean_end_to_end_difftest -- --ignored
```

Tests are `#[ignore]` by default (CI environments may lack the Lean toolchain).
