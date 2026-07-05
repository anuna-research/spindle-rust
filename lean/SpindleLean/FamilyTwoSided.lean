/-
  SpindleLean.FamilyTwoSided
  The two-sided (proved / disproved) fixed point: constructive defeat
  discard for the family model.

  The three-phase model (Family.lean) discards attackers only via the
  lambda over-approximation (`attackReaches`), which cannot express the
  spec's inductive discard `∃a ∈ body(s), -d a ∈ P` when the -d arises
  from a LOST BATTLE rather than unfoundedness. The engine implements
  the spec (`reason/defeasible.rs`, family-aware live-count discard);
  the smallest theory distinguishing the two semantics has 4 rules:

      r0: => p     r1: => ~p     r2: p ~> ~q     r3: => q

  Ambiguity defeats p (-d), so r2's premise is dead and r2 must be
  discarded, making q provable — which lambda-only discard cannot see
  (p ∈ lambda keeps r2 "reachable" forever).

  This module models the joint derivation of +d (P) and -d (N) as a
  monotone two-sided fixed point:

  - a literal is DEAD when it is outside lambda (unfounded) or in N
    (constructively disproven);
  - an atemporal body literal is dead only when EVERY family member in
    the universe is dead (mirrors the engine's family_live counting);
  - a rule is DISCARDED when some body literal is dead;
  - +d (canProve2): the delta-consistency gate, then a productive
    supporter with famSat body in P, and every attacker fact-exempt,
    discarded, or team-defeated by a superior P-applicable defender;
  - -d (canDisprove2): definite complement, unfoundedness, all
    supporters discarded, or an applicable attacker no superior
    applicable defender can beat (the spec's -d disjuncts).

  Both step components are monotone in (P, N), so fuel-bounded iteration
  reaches the least fixed point. `famReason2` is the executable model
  behind the family oracle and is difftested exhaustively against the
  engine — including a 4-rule propositional tier that covers the
  defeat-discard class (`lean_family_exhaustive_difftest.rs`).
-/
import SpindleLean.Family

namespace Family

/-! ## Death, discard, applicability -/

/-- A literal is dead when it can no longer be proven: outside the
    lambda over-approximation, or constructively disproven. -/
def deadLit (lambda N : List FLit) (m : FLit) : Bool :=
  !lambda.contains m || N.contains m

/-- A body literal is unsatisfiable-forever: a temporal literal must
    itself be dead; an atemporal literal is dead only when every family
    member in the universe is dead (the engine's family_live = 0). -/
def bodyDead (univ lambda N : List FLit) (b : FLit) : Bool :=
  if b.window == none then
    (univ.filter (fun m => FLit.sameFamily m b)).all (deadLit lambda N)
  else
    deadLit lambda N b

/-- A rule is discarded when some logic body literal is dead — the
    spec's condition (3) inductive discard, family-aware. -/
def discardedRule (univ lambda N : List FLit) (r : FRule) : Bool :=
  r.body.any (bodyDead univ lambda N)

/-- Team defeat wrt the proven set. -/
def teamDefeats2 (t : FTheory) (lit : FLit) (attacker : FRule)
    (P : List FLit) : Bool :=
  (t.rulesWithHead lit).any fun d =>
    d.isProductive && d.bodySat P && t.isSuperior d.label attacker.label

/-! ## The two inference conditions -/

/-- +d: the delta-consistency gate, subsumption, or supported with all
    attacks answered (discarded or beaten). -/
def canProve2 (t : FTheory) (univ delta lambda P N : List FLit)
    (lit : FLit) : Bool :=
  if delta.contains lit.complement then false
  else if delta.contains lit then true
  else
    ((t.rulesWithHead lit).any fun r => r.isProductive && r.bodySat P)
      && (t.rulesWithHead lit.complement).all fun s =>
        s.isFact
          || discardedRule univ lambda N s
          || teamDefeats2 t lit s P

/-- -d: definite complement, unfounded, unsupported (all supporters
    discarded), or attacked by an applicable rule that no superior
    applicable defender beats. -/
def canDisprove2 (t : FTheory) (univ delta lambda P N : List FLit)
    (lit : FLit) : Bool :=
  delta.contains lit.complement
    || !lambda.contains lit
    || ((t.rulesWithHead lit).all fun r =>
          !r.isProductive || discardedRule univ lambda N r)
    || ((t.rulesWithHead lit.complement).any fun s =>
          !s.isFact && s.bodySat P
            && !((t.rulesWithHead lit).any fun d =>
                  d.isProductive && d.bodySat P
                    && t.isSuperior d.label s.label))

/-! ## The two-sided step and closure -/

/-- One joint step: extend P with newly provable lambda candidates and
    N with newly disprovable universe literals. Both components only
    grow. -/
def twoSidedStep (t : FTheory) (univ delta lambda : List FLit)
    (P N : List FLit) : List FLit × List FLit :=
  let P' := (P ++ lambda.filter fun l =>
    !P.contains l && canProve2 t univ delta lambda P N l).dedup
  let N' := (N ++ univ.filter fun l =>
    !N.contains l && !P'.contains l
      && canDisprove2 t univ delta lambda P N l).dedup
  (P', N')

/-- Iterate to the fixed point (fuel-bounded; each productive round adds
    at least one literal to P or N, so 2·|univ|+2 rounds suffice). -/
def twoSidedClose (t : FTheory) (univ delta lambda : List FLit)
    (P N : List FLit) : Nat → List FLit × List FLit
  | 0 => (P, N)
  | fuel + 1 =>
    let (P', N') := twoSidedStep t univ delta lambda P N
    if P'.length == P.length && N'.length == N.length then (P, N)
    else twoSidedClose t univ delta lambda P' N' fuel

/-- The two-sided family reasoning pipeline: delta and lambda as in the
    three-phase model, then the joint (+d, -d) fixed point. Returns
    (delta, proven). Final -d = universe \ proven (the engine's Phase-3
    sweep). -/
def famReason2 (t : FTheory) : List FLit × List FLit :=
  let delta := deltaCloseWith FRule.bodySat t
  let lambda := lambdaCloseWith FRule.bodySat t delta
  let univ := t.allLiterals
  let fuel := 2 * univ.length + 2
  let (p, _) := twoSidedClose t univ delta lambda (gatedDeltaF delta) [] fuel
  (delta, p)

/-! ## Basic structural properties -/

theorem twoSidedStep_P_extends (t : FTheory) (univ delta lambda P N : List FLit) :
    ∀ l ∈ P, l ∈ (twoSidedStep t univ delta lambda P N).1 := by
  intro l hl
  simp only [twoSidedStep, List.mem_dedup, List.mem_append]
  exact Or.inl hl

theorem twoSidedStep_N_extends (t : FTheory) (univ delta lambda P N : List FLit) :
    ∀ l ∈ N, l ∈ (twoSidedStep t univ delta lambda P N).2 := by
  intro l hl
  simp only [twoSidedStep, List.mem_dedup, List.mem_append]
  exact Or.inl hl

/-- New +d candidates are drawn from lambda: the proven set stays inside
    the over-approximation whenever it starts there. -/
theorem twoSidedStep_P_subset_lambda (t : FTheory)
    (univ delta lambda P N : List FLit)
    (hP : ∀ l ∈ P, l ∈ lambda) :
    ∀ l ∈ (twoSidedStep t univ delta lambda P N).1, l ∈ lambda := by
  intro l hl
  simp only [twoSidedStep, List.mem_dedup, List.mem_append] at hl
  rcases hl with h | h
  · exact hP l h
  · exact (List.mem_filter.mp h).1

end Family
