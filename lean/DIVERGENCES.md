# Divergences: Rust Engine vs Verified Lean Model — RESOLVED

> **Resolution (2026-07-04).** Both classes below are resolved; the engine
> and the verified model now agree on **all 297,760 theories** at full
> exhaustive scope (0 divergences).
>
> - **Class 1** — resolved by adopting the engine's paraconsistent gate in
>   the Lean model (`Closure/Partial.lean`: `canProve` checks the
>   complement-in-delta condition first; `partialClose` seeds from
>   `gatedDelta`). `delta_subset_partial` and `faithful_D_implies_d` are now
>   conditional on delta-consistency, and the spec's formal `+d` clause was
>   corrected to match its own worked example 1. Payoff: the new
>   `Properties/Consistency.lean` proves **`partial_consistent`** — the
>   defeasible level never contains a complementary pair, for ANY
>   well-formed input whose superiority relation is well-founded. Standard
>   Antoniou et al. DL cannot have this theorem.
> - **Class 2** — resolved by adopting the model's well-founded
>   lambda-discard in the Rust engine (`reason/defeasible.rs`:
>   `compute_lambda` + seeding `-d` for literals outside lambda), and
>   removing an unspecified "strict attackers cannot be beaten by
>   superiority" carve-out that contradicted spec condition (3). The spec
>   gained a "well-founded strengthening" section documenting the lambda
>   reading.
>
> The analysis below is retained as the historical record of what the
> exhaustive difftest found and why each decision was taken.
>
> **Class 3 (2026-07-05, temporal-family difftest — RESOLVED).** The
> family difftest (`lean_family_exhaustive_difftest.rs`) found an
> order-dependent discard in the worklist: a `-d` event for an exact
> atemporal literal discarded rules whose atemporal body was still
> family-satisfiable by a temporal member (e.g. `~p -> p` with
> `=> ~p[1,10]`), so outcomes depended on whether the family supporter
> was seeded before the discard (facts won the race; defeasible
> supporters lost it). Fixed by family-aware discard in
> `reason/defeasible.rs`: a `-d` event only discards a rule when it
> removes the LAST way to satisfy a body literal — exact match for
> temporal bodies; whole-family-unfounded (no member in lambda) for
> atemporal bodies. After the fix: 400,730 temporal-family theories at
> full scope, zero divergences.
>
> **Class-3 refinement (2026-07-05, review finding).** The first fix
> guarded discards by lambda-aliveness alone, which over-protected:
> a premise that is lambda-alive but ACTUALLY DEFEATED (e.g. loses a
> superiority battle) kept its dependent attackers alive, blocking valid
> conclusions — a regression vs the spec's condition (3) inductive
> discard, with a minimal witness of 5 rules (outside the exhaustive
> scope; caught in review). Refined to LIVE-member counting: a family's
> live support = lambda members not yet disproven, decremented on defeat
> events; atemporal-bodied rules are discarded exactly when their
> family's live count reaches zero. Regression test:
> `test_defeated_premise_discards_dependent_attacker`.
>
> **Two-sided fixed point (2026-07-05) — the model gap CLOSED, and a
> fourth engine bug found.** `SpindleLean/FamilyTwoSided.lean` models the
> joint (+d, -d) derivation as a monotone two-sided fixed point with
> constructive defeat-discard (the spec's `∃a ∈ body(s), -d a ∈ P`,
> family-aware), replacing the lambda-only `attackReaches`
> over-approximation. The family oracle now runs this model, and the
> family difftest gained a 4-rule propositional tier covering the
> defeat-discard class (minimal witness: `=> p`, `=> ~p`, `p ~> ~q`,
> `=> q` — no superiority needed; ambiguity defeats p).
>
> The new tier immediately found **class 4**: the engine's Phase-2
> worklist was purely event-driven, so battles among rules that never
> received a triggering event (competing empty-body rules in a fact-free
> theory) stayed undecided — the spec-derivable `-d p` (disjunct (3):
> applicable unbeaten attacker) was never derived constructively, its
> dependent discard never applied, and `+d q` was wrongly withheld. This
> affected main too (it was not introduced by the class-3 work). Fixed
> with battle-resolution sweeps: after the event queue drains, every
> undecided literal is re-tried against the +d/-d conditions until no
> new decision appears. The sweep fix also surfaced 36 three-rule
> superiority battles (e.g. `p -> p`, `~p -> p`, `=> ~p`, `r2 > r1`)
> where the spec derives `-d p` immediately (no support beats the
> attacker) and hence `+d ~p` — the engine now derives them and the
> lambda-only three-phase model does not. The exhaustive difftests
> therefore compare the engine against the TWO-SIDED model as the
> operational reference; the three-phase model remains the carrier of
> the property proofs, documented as a lambda-only approximation that
> under-derives on fact-free battles. After the fixes: 1,359,936
> theories at full scope across both suites (SDL 323,830 + family
> 1,036,106, the latter including 635,376 four-rule propositional
> cases), zero divergences.

# Known Divergences: Rust Engine vs Verified Lean Model (historical)

Found by the exhaustive small-scope differential test
(`crates/spindle-core/tests/lean_sdl_exhaustive_difftest.rs`), which
enumerates **every** propositional theory over 2 atoms, bodies ≤ 1 literal,
up to 3 rules, and superiority pairs, and compares the Rust engine
(`spindle_core::reason::reason`) against the Lean SDL oracle
(`spindlelean --oracle-batch`) literal-by-literal on +D / -D / +d / -d.

**Headline result (297,760 theories at `SPINDLE_EXHAUSTIVE=full` scope,
2026-07-04):**

- **+D / -D agree on every theory.** The definite level is in perfect
  correspondence.
- Every divergence is defeasible-level (+d / -d) and falls into exactly
  the two classes below (7,300 class-1 and 3,436 class-2 instances).
  No third class exists in scope.

Both classes trace to a genuine ambiguity in
[[DEFEASIBLE-LOGIC-SEMANTICS]] (`specs/DEFEASIBLE-LOGIC-SEMANTICS.md`),
whose *formal clauses* and *worked examples / engine behavior* disagree.
Resolving them is a semantics decision, not a bug fix — recorded here per
the anti-slop discipline until the decision lands.

---

## Class 1 — Inconsistent-delta subsumption (7,300 cases in full scope)

**Minimal witness:**

```
f1: >> p
f2: >> ~p
```

| | +D p | +D ~p | +d p | +d ~p |
|---|---|---|---|---|
| Rust engine | ✔ | ✔ | ✘ (-d) | ✘ (-d) |
| Lean model | ✔ | ✔ | ✔ | ✔ |

**Root cause:** the spec's formal clause says `+d q iff +D q ∈ P OR (…)` —
unconditional subsumption, which is the standard Antoniou/Billington/
Governatori/Maher definition and what the Lean model implements
(`delta_subset_partial`, `faithful_D_implies_d`). But the spec's Worked
Example 1 gates the subsumption on `-D ~q` ("consistency enforced by
condition (2)") and concludes `-d p, -d ~p`; the Rust engine implements
the worked example (`reason/defeasible.rs`: "+D q yields +d q only if
-D ~q").

**The spec contradicts itself.** Standard DL propagates strict
inconsistency into the defeasible level (garbage in, garbage out); the
worked-example variant is deliberately paraconsistent.

**Resolution options:**
1. *Align Lean to the engine* (gate subsumption on `-D ~q`): matches the
   shipping behavior and the worked example; requires reworking
   `Closure/Partial.lean` and conditioning `delta_subset_partial` /
   `faithful_D_implies_d` and the Faithfulness proofs on delta-consistency.
2. *Align the engine to the formal clause* (standard DL): one-line-ish
   change in `reason/defeasible.rs` seeding, plus updating the worked
   example and the SDL conformance tests.

Either way, `specs/DEFEASIBLE-LOGIC-SEMANTICS.md` must be corrected so its
formal clause and worked example agree.

## Class 2 — Circular-attacker discard (3,436 cases in full scope)

**Minimal witness:**

```
r0: p => p
r1: => ~p
```

| | +d ~p | -d p |
|---|---|---|
| Rust engine | ✘ (-d ~p) | ✔ (via negative sweep) |
| Lean model | ✔ | ✔ |

**Root cause:** to prove `+d ~p`, every rule for `p` must be discarded or
beaten. `r0`'s body is `{p}` — circular, never provable. The Lean model
(DL(d‖) three-phase) discards attackers whose bodies fall outside the
**lambda over-approximation**; `p` never enters lambda, so `r0` is
discarded and `+d ~p` follows. The Rust engine's constructive worklist
never *derives* `-d p` (the inductive `-d` inference for `r0` needs
`-d p` itself), so `r0` stays undecided, which conservatively blocks
`~p`; the Phase-3 sweep then reports both as `-d`.

The paper's inductive proof theory leaves such literals **undecided**
(neither `+∂` nor `-∂` derivable); both implementations totalize, in
opposite directions. Lean's reading is well-founded-style ("an unfounded
attacker cannot block"); Rust's is conservative ("an undecided attacker
blocks"). The divergence is strictly one-directional in scope: Lean's
`+d` set is always a superset of Rust's.

**Resolution options:**
1. *Align Lean to the engine*: replace the lambda-based discard with a
   constructive two-sided (proved/disproved) fixed point mirroring
   `reason/defeasible.rs` — a substantial model rewrite (the current
   three-phase model has no explicit negative set).
2. *Align the engine to the model*: discard attackers whose bodies cannot
   appear in the lambda over-approximation — makes the engine strictly
   more decisive on loops; contradicts the spec's inductive `-d` clause
   as written.
3. *Accept and document*: treat lambda-discard as the model's known
   over-approximation; the difftest permanently tolerates the class (its
   one-directionality is machine-checked on every run).

---

## Status

**Resolved** — see the header. The exhaustive difftest now fails on any
divergence whatsoever; the tolerance machinery has been removed.
