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
