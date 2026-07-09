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

/-- The proven-side step: extend P with newly provable lambda candidates
    (never already-disproven ones, mirroring the engine's decided-guard). -/
def twoSidedStepP (t : FTheory) (univ delta lambda : List FLit)
    (P N : List FLit) : List FLit :=
  (P ++ lambda.filter fun l =>
    !P.contains l && !N.contains l
      && canProve2 t univ delta lambda P N l).dedup

/-- The disproven-side step: extend N with newly disprovable universe
    literals not claimed by the proven side this round.

    `canDisprove2` is evaluated against the NEW proven set `P'`
    (`twoSidedStepP` this round), not the round's input `P`. This mirrors the
    engine's incremental worklist: a literal must not be disproven while a
    (possibly strict, possibly superior) defender's body becomes provable in
    the same round — e.g. `q -> p`, `=> ~p`, `=> q`, `r0 > r1`, where `q`
    enters P and defends `p` in one round. Because N only grows, evaluating
    against the stale `P` would disprove `p` permanently before its defense
    materialized. -/
def twoSidedStepN (t : FTheory) (univ delta lambda : List FLit)
    (P N : List FLit) : List FLit :=
  let P' := twoSidedStepP t univ delta lambda P N
  (N ++ univ.filter fun l =>
    !N.contains l && !P'.contains l
      && canDisprove2 t univ delta lambda P' N l).dedup

/-- One joint step. Both components only grow. -/
def twoSidedStep (t : FTheory) (univ delta lambda : List FLit)
    (P N : List FLit) : List FLit × List FLit :=
  (twoSidedStepP t univ delta lambda P N, twoSidedStepN t univ delta lambda P N)

/-- Iterate to the fixed point (fuel-bounded; each productive round adds
    at least one literal to P or N, so 2·|univ|+2 rounds suffice). -/
def twoSidedClose (t : FTheory) (univ delta lambda : List FLit)
    (P N : List FLit) : Nat → List FLit × List FLit
  | 0 => (P, N)
  | fuel + 1 =>
    if (twoSidedStep t univ delta lambda P N).1.length == P.length
        && (twoSidedStep t univ delta lambda P N).2.length == N.length then
      (P, N)
    else
      twoSidedClose t univ delta lambda
        (twoSidedStep t univ delta lambda P N).1
        (twoSidedStep t univ delta lambda P N).2 fuel

/-- The two-sided family reasoning pipeline: delta and lambda as in the
    three-phase model, then the joint (+d, -d) fixed point. Returns
    (delta, proven). Final -d = universe \ proven (the engine's Phase-3
    sweep). -/
def famReason2 (t : FTheory) : List FLit × List FLit :=
  (deltaCloseWith FRule.bodySat t,
    (twoSidedClose t t.allLiterals
      (deltaCloseWith FRule.bodySat t)
      (lambdaCloseWith FRule.bodySat t (deltaCloseWith FRule.bodySat t))
      (gatedDeltaF (deltaCloseWith FRule.bodySat t)) []
      (2 * t.allLiterals.length + 2)).1)

/-! ## Basic structural properties -/

theorem twoSidedStep_P_extends (t : FTheory) (univ delta lambda P N : List FLit) :
    ∀ l ∈ P, l ∈ (twoSidedStep t univ delta lambda P N).1 := by
  intro l hl
  simp only [twoSidedStep, twoSidedStepP, List.mem_dedup, List.mem_append]
  exact Or.inl hl

theorem twoSidedStep_N_extends (t : FTheory) (univ delta lambda P N : List FLit) :
    ∀ l ∈ N, l ∈ (twoSidedStep t univ delta lambda P N).2 := by
  intro l hl
  simp only [twoSidedStep, twoSidedStepN, List.mem_dedup, List.mem_append]
  exact Or.inl hl

/-- New +d candidates are drawn from lambda: the proven set stays inside
    the over-approximation whenever it starts there. -/
theorem twoSidedStep_P_subset_lambda (t : FTheory)
    (univ delta lambda P N : List FLit)
    (hP : ∀ l ∈ P, l ∈ lambda) :
    ∀ l ∈ (twoSidedStep t univ delta lambda P N).1, l ∈ lambda := by
  intro l hl
  simp only [twoSidedStep, twoSidedStepP, List.mem_dedup, List.mem_append] at hl
  rcases hl with h | h
  · exact hP l h
  · exact (List.mem_filter.mp h).1

/-! ## Monotonicity -/

theorem famSat_mono {P P' : List FLit} (hsub : ∀ x ∈ P, x ∈ P') (l : FLit)
    (h : FLit.famSat P l = true) : FLit.famSat P' l = true := by
  rw [FLit.famSat_iff] at h ⊢
  rcases h with h | ⟨hw, m, hm, hfam⟩
  · exact Or.inl (hsub l h)
  · exact Or.inr ⟨hw, m, hsub m hm, hfam⟩

theorem bodySat_mono (r : FRule) {P P' : List FLit}
    (hsub : ∀ x ∈ P, x ∈ P') (h : r.bodySat P = true) :
    r.bodySat P' = true := by
  simp only [FRule.bodySat, List.all_eq_true] at h ⊢
  exact fun b hb => famSat_mono hsub b (h b hb)

theorem deadLit_mono (lambda : List FLit) {N N' : List FLit}
    (hsub : ∀ x ∈ N, x ∈ N') (m : FLit)
    (h : deadLit lambda N m = true) : deadLit lambda N' m = true := by
  simp only [deadLit, Bool.or_eq_true] at h ⊢
  rcases h with h | h
  · exact Or.inl h
  · exact Or.inr (List.contains_iff_mem.mpr (hsub m (List.contains_iff_mem.mp h)))

theorem bodyDead_mono (univ lambda : List FLit) {N N' : List FLit}
    (hsub : ∀ x ∈ N, x ∈ N') (b : FLit)
    (h : bodyDead univ lambda N b = true) : bodyDead univ lambda N' b = true := by
  simp only [bodyDead] at h ⊢
  split at h <;> rename_i hw
  · rw [if_pos hw]
    simp only [List.all_eq_true] at h ⊢
    exact fun m hm => deadLit_mono lambda hsub m (h m hm)
  · rw [if_neg hw]
    exact deadLit_mono lambda hsub b h

theorem discardedRule_mono (univ lambda : List FLit) {N N' : List FLit}
    (hsub : ∀ x ∈ N, x ∈ N') (r : FRule)
    (h : discardedRule univ lambda N r = true) :
    discardedRule univ lambda N' r = true := by
  simp only [discardedRule, List.any_eq_true] at h ⊢
  obtain ⟨b, hb, hd⟩ := h
  exact ⟨b, hb, bodyDead_mono univ lambda hsub b hd⟩

theorem teamDefeats2_mono (t : FTheory) (lit : FLit) (attacker : FRule)
    {P P' : List FLit} (hsub : ∀ x ∈ P, x ∈ P')
    (h : teamDefeats2 t lit attacker P = true) :
    teamDefeats2 t lit attacker P' = true := by
  simp only [teamDefeats2, List.any_eq_true] at h ⊢
  obtain ⟨d, hd, hcond⟩ := h
  simp only [Bool.and_eq_true] at hcond ⊢
  exact ⟨d, hd, ⟨hcond.1.1, bodySat_mono d hsub hcond.1.2⟩, hcond.2⟩

/-- `canProve2` is monotone in the proven and disproven sets. -/
theorem canProve2_mono (t : FTheory) (univ delta lambda : List FLit)
    {P P' N N' : List FLit}
    (hP : ∀ x ∈ P, x ∈ P') (hN : ∀ x ∈ N, x ∈ N') (lit : FLit)
    (h : canProve2 t univ delta lambda P N lit = true) :
    canProve2 t univ delta lambda P' N' lit = true := by
  unfold canProve2 at h ⊢
  split at h
  · exact absurd h Bool.false_ne_true
  · rename_i hgate
    rw [if_neg hgate]
    split at h
    · rename_i hd
      rw [if_pos hd]
    · rename_i hd
      rw [if_neg hd]
      simp only [Bool.and_eq_true, List.any_eq_true, List.all_eq_true] at h ⊢
      obtain ⟨⟨r, hr, hcond⟩, hatt⟩ := h
      refine ⟨⟨r, hr, ?_⟩, ?_⟩
      · exact ⟨hcond.1, bodySat_mono r hP hcond.2⟩
      · intro s hs
        have hthis := hatt s hs
        rw [Bool.or_eq_true, Bool.or_eq_true] at hthis ⊢
        rcases hthis with (hf | hdisc) | hteam
        · exact Or.inl (Or.inl hf)
        · exact Or.inl (Or.inr (discardedRule_mono univ lambda hN s hdisc))
        · exact Or.inr (teamDefeats2_mono t lit s hP hteam)

/-! ## Equation lemmas for the closure -/

theorem twoSidedClose_fixpoint_eq (t : FTheory)
    (univ delta lambda P N : List FLit) (n : Nat)
    (hlen : ((twoSidedStep t univ delta lambda P N).1.length == P.length
      && (twoSidedStep t univ delta lambda P N).2.length == N.length) = true) :
    twoSidedClose t univ delta lambda P N (n + 1) = (P, N) := by
  simp only [twoSidedClose]
  rw [if_pos hlen]

theorem twoSidedClose_step_eq (t : FTheory)
    (univ delta lambda P N : List FLit) (n : Nat)
    (hlen : ¬ ((twoSidedStep t univ delta lambda P N).1.length == P.length
      && (twoSidedStep t univ delta lambda P N).2.length == N.length) = true) :
    twoSidedClose t univ delta lambda P N (n + 1)
      = twoSidedClose t univ delta lambda
          (twoSidedStep t univ delta lambda P N).1
          (twoSidedStep t univ delta lambda P N).2 n := by
  simp only [twoSidedClose]
  rw [if_neg hlen]

/-! ## Closure invariants -/

theorem twoSidedClose_P_extends (t : FTheory) (univ delta lambda : List FLit)
    (P N : List FLit) (fuel : Nat) :
    ∀ l ∈ P, l ∈ (twoSidedClose t univ delta lambda P N fuel).1 := by
  induction fuel generalizing P N with
  | zero => intro l hl; exact hl
  | succ n ih =>
    intro l hl
    by_cases hlen : ((twoSidedStep t univ delta lambda P N).1.length == P.length
        && (twoSidedStep t univ delta lambda P N).2.length == N.length) = true
    · rw [twoSidedClose_fixpoint_eq t univ delta lambda P N n hlen]
      exact hl
    · rw [twoSidedClose_step_eq t univ delta lambda P N n hlen]
      exact ih _ _ l (twoSidedStep_P_extends t univ delta lambda P N l hl)

theorem twoSidedClose_N_extends (t : FTheory) (univ delta lambda : List FLit)
    (P N : List FLit) (fuel : Nat) :
    ∀ l ∈ N, l ∈ (twoSidedClose t univ delta lambda P N fuel).2 := by
  induction fuel generalizing P N with
  | zero => intro l hl; exact hl
  | succ n ih =>
    intro l hl
    by_cases hlen : ((twoSidedStep t univ delta lambda P N).1.length == P.length
        && (twoSidedStep t univ delta lambda P N).2.length == N.length) = true
    · rw [twoSidedClose_fixpoint_eq t univ delta lambda P N n hlen]
      exact hl
    · rw [twoSidedClose_step_eq t univ delta lambda P N n hlen]
      exact ih _ _ l (twoSidedStep_N_extends t univ delta lambda P N l hl)

theorem twoSidedClose_P_subset_lambda (t : FTheory)
    (univ delta lambda : List FLit) (P N : List FLit) (fuel : Nat)
    (hP : ∀ l ∈ P, l ∈ lambda) :
    ∀ l ∈ (twoSidedClose t univ delta lambda P N fuel).1, l ∈ lambda := by
  induction fuel generalizing P N with
  | zero => exact hP
  | succ n ih =>
    intro l hl
    by_cases hlen : ((twoSidedStep t univ delta lambda P N).1.length == P.length
        && (twoSidedStep t univ delta lambda P N).2.length == N.length) = true
    · rw [twoSidedClose_fixpoint_eq t univ delta lambda P N n hlen] at hl
      exact hP l hl
    · rw [twoSidedClose_step_eq t univ delta lambda P N n hlen] at hl
      exact ih _ _ (twoSidedStep_P_subset_lambda t univ delta lambda P N hP) l hl

/-- One step preserves disjointness of the proven and disproven sets. -/
theorem twoSidedStep_disjoint (t : FTheory) (univ delta lambda P N : List FLit)
    (hdisj : ∀ l ∈ P, l ∉ N) :
    ∀ l ∈ (twoSidedStep t univ delta lambda P N).1,
      l ∉ (twoSidedStep t univ delta lambda P N).2 := by
  intro l hlP hlN
  have hlP2 : l ∈ twoSidedStepP t univ delta lambda P N := hlP
  have hlN2 : l ∈ twoSidedStepN t univ delta lambda P N := hlN
  simp only [twoSidedStepN, List.mem_dedup, List.mem_append,
    List.mem_filter, Bool.and_eq_true, Bool.not_eq_true'] at hlN2
  rcases hlN2 with hInN | ⟨_, ⟨_, hnotP2⟩, _⟩
  · simp only [twoSidedStepP, List.mem_dedup, List.mem_append,
      List.mem_filter, Bool.and_eq_true, Bool.not_eq_true'] at hlP2
    rcases hlP2 with hInP | ⟨_, ⟨_, hnN⟩, _⟩
    · exact hdisj l hInP hInN
    · have hc : N.contains l = true := List.contains_iff_mem.mpr hInN
      rw [hc] at hnN
      exact Bool.noConfusion hnN
  · have hc : (twoSidedStepP t univ delta lambda P N).contains l = true :=
      List.contains_iff_mem.mpr hlP2
    rw [hc] at hnotP2
    exact Bool.noConfusion hnotP2

theorem twoSidedClose_disjoint (t : FTheory) (univ delta lambda : List FLit)
    (P N : List FLit) (fuel : Nat) (hdisj : ∀ l ∈ P, l ∉ N) :
    ∀ l ∈ (twoSidedClose t univ delta lambda P N fuel).1,
      l ∉ (twoSidedClose t univ delta lambda P N fuel).2 := by
  induction fuel generalizing P N with
  | zero => exact hdisj
  | succ n ih =>
    intro l hl
    by_cases hlen : ((twoSidedStep t univ delta lambda P N).1.length == P.length
        && (twoSidedStep t univ delta lambda P N).2.length == N.length) = true
    · rw [twoSidedClose_fixpoint_eq t univ delta lambda P N n hlen] at hl ⊢
      exact hdisj l hl
    · rw [twoSidedClose_step_eq t univ delta lambda P N n hlen] at hl ⊢
      exact ih _ _ (twoSidedStep_disjoint t univ delta lambda P N hdisj) l hl

/-! ## Soundness at the final state -/

/-- Every member of the final proven set satisfies `canProve2` evaluated
    at the final state: seeds by hypothesis, step-added members by their
    add-time witness transported forward by monotonicity. Mirrors
    `partial_go_sound`. -/
theorem twoSidedClose_P_sound (t : FTheory) (univ delta lambda : List FLit)
    (P N : List FLit) (fuel : Nat)
    (hseed : ∀ x ∈ P,
      canProve2 t univ delta lambda
        (twoSidedClose t univ delta lambda P N fuel).1
        (twoSidedClose t univ delta lambda P N fuel).2 x = true) :
    ∀ l ∈ (twoSidedClose t univ delta lambda P N fuel).1,
      canProve2 t univ delta lambda
        (twoSidedClose t univ delta lambda P N fuel).1
        (twoSidedClose t univ delta lambda P N fuel).2 l = true := by
  induction fuel generalizing P N with
  | zero =>
    intro l hl
    exact hseed l hl
  | succ n ih =>
    by_cases hlen :
        ((twoSidedStep t univ delta lambda P N).1.length == P.length
          && (twoSidedStep t univ delta lambda P N).2.length == N.length) = true
    · intro l hl
      rw [twoSidedClose_fixpoint_eq t univ delta lambda P N n hlen] at hl hseed ⊢
      exact hseed l hl
    · intro l hl
      rw [twoSidedClose_step_eq t univ delta lambda P N n hlen] at hl hseed ⊢
      refine ih _ _ ?_ l hl
      intro x hx
      -- x comes from P (seed hypothesis, already stated at the final
      -- state of this recursion) or was added with a witness at (P, N).
      have hx2 : x ∈ twoSidedStepP t univ delta lambda P N := hx
      simp only [twoSidedStepP, List.mem_dedup, List.mem_append,
        List.mem_filter, Bool.and_eq_true] at hx2
      rcases hx2 with hxP | ⟨_, _, hcan⟩
      · exact hseed x hxP
      · -- transport the (P, N) witness to the final state
        have hPfin : ∀ y ∈ P,
            y ∈ (twoSidedClose t univ delta lambda
              (twoSidedStep t univ delta lambda P N).1
              (twoSidedStep t univ delta lambda P N).2 n).1 := fun y hy =>
          twoSidedClose_P_extends t univ delta lambda _ _ n y
            (twoSidedStep_P_extends t univ delta lambda P N y hy)
        have hNfin : ∀ y ∈ N,
            y ∈ (twoSidedClose t univ delta lambda
              (twoSidedStep t univ delta lambda P N).1
              (twoSidedStep t univ delta lambda P N).2 n).2 := fun y hy =>
          twoSidedClose_N_extends t univ delta lambda _ _ n y
            (twoSidedStep_N_extends t univ delta lambda P N y hy)
        exact canProve2_mono t univ delta lambda hPfin hNfin x hcan

/-! ## Helper facts for consistency -/

/-- Seed members (gated delta) satisfy `canProve2` at every state. -/
theorem gatedDelta_canProve2 (t : FTheory) (univ delta lambda : List FLit)
    (l : FLit) (hl : l ∈ gatedDeltaF delta) :
    ∀ Q M : List FLit, canProve2 t univ delta lambda Q M l = true := by
  intro Q M
  simp only [gatedDeltaF, List.mem_filter, Bool.not_eq_true'] at hl
  obtain ⟨hmem, hnc⟩ := hl
  unfold canProve2
  rw [if_neg (by rw [hnc]; exact Bool.false_ne_true),
      if_pos (List.contains_iff_mem.mpr hmem)]

/-- Fact heads are definite: they form the seed of the delta closure. -/
theorem fact_head_mem_famDelta (t : FTheory) (s : FRule) (hs : s ∈ t.rules)
    (hfact : s.isFact = true) :
    s.head ∈ deltaCloseWith FRule.bodySat t := by
  have hseed : s.head ∈
      ((t.rules.filter (fun r => r.ruleType == FRuleType.fact)).map
        (fun r => r.head)).dedup := by
    rw [List.mem_dedup, List.mem_map]
    refine ⟨s, ?_, rfl⟩
    rw [List.mem_filter]
    exact ⟨hs, hfact⟩
  -- go only extends its input
  have hgo : ∀ (current : List FLit) (fuel : Nat), ∀ x ∈ current,
      x ∈ deltaCloseWith.go FRule.bodySat t current fuel := by
    intro current fuel
    induction fuel generalizing current with
    | zero => intro x hx; exact hx
    | succ n ih =>
      intro x hx
      simp only [deltaCloseWith.go]
      split
      · exact hx
      · refine ih _ x ?_
        simp only [deltaStepWith, List.mem_dedup, List.mem_append]
        exact Or.inl hx
  exact hgo _ 1000 s.head hseed

/-- The delta closure stays within the theory's literal universe. -/
theorem famDelta_subset_univ (t : FTheory) :
    ∀ l ∈ deltaCloseWith FRule.bodySat t, l ∈ t.allLiterals := by
  have hstep : ∀ (current : List FLit),
      (∀ x ∈ current, x ∈ t.allLiterals) →
      ∀ x ∈ deltaStepWith FRule.bodySat t current, x ∈ t.allLiterals := by
    intro current hsub x hx
    simp only [deltaStepWith, List.mem_dedup, List.mem_append] at hx
    rcases hx with h | h
    · exact hsub x h
    · simp only [List.mem_filterMap] at h
      obtain ⟨r, hr, hcond⟩ := h
      split at hcond
      · cases hcond
        simp only [FTheory.allLiterals, List.mem_dedup, List.mem_append]
        exact Or.inl (List.mem_map.mpr ⟨r, hr, rfl⟩)
      · cases hcond
  have hseed : ∀ x ∈
      ((t.rules.filter (fun r => r.ruleType == FRuleType.fact)).map
        (fun r => r.head)).dedup, x ∈ t.allLiterals := by
    intro x hx
    rw [List.mem_dedup, List.mem_map] at hx
    obtain ⟨r, hr, rfl⟩ := hx
    rw [List.mem_filter] at hr
    simp only [FTheory.allLiterals, List.mem_dedup, List.mem_append]
    exact Or.inl (List.mem_map.mpr ⟨r, hr.1, rfl⟩)
  have hgo : ∀ (current : List FLit) (fuel : Nat),
      (∀ x ∈ current, x ∈ t.allLiterals) →
      ∀ x ∈ deltaCloseWith.go FRule.bodySat t current fuel,
        x ∈ t.allLiterals := by
    intro current fuel
    induction fuel generalizing current with
    | zero => intro h x hx; exact h x hx
    | succ n ih =>
      intro h x hx
      simp only [deltaCloseWith.go] at hx
      split at hx
      · exact h x hx
      · exact ih _ (hstep current h) x hx
  exact hgo _ 1000 hseed

/-- The lambda closure stays within the theory's literal universe. -/
theorem famLambda_subset_univ (t : FTheory) (delta : List FLit)
    (hdelta : ∀ l ∈ delta, l ∈ t.allLiterals) :
    ∀ l ∈ lambdaCloseWith FRule.bodySat t delta, l ∈ t.allLiterals := by
  have hstep : ∀ (current : List FLit),
      (∀ x ∈ current, x ∈ t.allLiterals) →
      ∀ x ∈ lambdaStepWith FRule.bodySat t delta current,
        x ∈ t.allLiterals := by
    intro current hsub x hx
    simp only [lambdaStepWith, List.mem_dedup, List.mem_append] at hx
    rcases hx with h | h
    · exact hsub x h
    · simp only [List.mem_filterMap] at h
      obtain ⟨r, hr, hcond⟩ := h
      split at hcond
      · cases hcond
        simp only [FTheory.allLiterals, List.mem_dedup, List.mem_append]
        exact Or.inl (List.mem_map.mpr ⟨r, hr, rfl⟩)
      · cases hcond
  have hgo : ∀ (current : List FLit) (fuel : Nat),
      (∀ x ∈ current, x ∈ t.allLiterals) →
      ∀ x ∈ lambdaCloseWith.go FRule.bodySat t delta current fuel,
        x ∈ t.allLiterals := by
    intro current fuel
    induction fuel generalizing current with
    | zero => intro h x hx; exact h x hx
    | succ n ih =>
      intro h x hx
      simp only [lambdaCloseWith.go] at hx
      split at hx
      · exact h x hx
      · exact ih _ (hstep current h) x hx
  exact hgo _ 1000 hdelta

/-- Delta seeds the lambda closure. -/
theorem famDelta_subset_lambda (t : FTheory) (delta : List FLit) :
    ∀ l ∈ delta, l ∈ lambdaCloseWith FRule.bodySat t delta := by
  have hgo : ∀ (current : List FLit) (fuel : Nat), ∀ x ∈ current,
      x ∈ lambdaCloseWith.go FRule.bodySat t delta current fuel := by
    intro current fuel
    induction fuel generalizing current with
    | zero => intro x hx; exact hx
    | succ n ih =>
      intro x hx
      simp only [lambdaCloseWith.go]
      split
      · exact hx
      · refine ih _ x ?_
        simp only [lambdaStepWith, List.mem_dedup, List.mem_append]
        exact Or.inl hx
  exact hgo _ 1000

/-- A body literal satisfied in the proven set is never dead, provided the
    proven set lives inside lambda, avoids N, and lies in the universe. -/
theorem not_bodyDead_of_famSat (univ lambda N P : List FLit)
    (hPl : ∀ x ∈ P, x ∈ lambda) (hPu : ∀ x ∈ P, x ∈ univ)
    (hPN : ∀ x ∈ P, x ∉ N) (b : FLit)
    (hsat : FLit.famSat P b = true) :
    bodyDead univ lambda N b = false := by
  have not_dead : ∀ w ∈ P, deadLit lambda N w = false := by
    intro w hw
    simp only [deadLit]
    have h1 : lambda.contains w = true := List.contains_iff_mem.mpr (hPl w hw)
    have h2 : N.contains w = false := by
      cases hc : N.contains w
      · rfl
      · exact absurd (List.contains_iff_mem.mp hc) (hPN w hw)
    rw [h1, h2]
    rfl
  rw [FLit.famSat_iff] at hsat
  simp only [bodyDead]
  rcases hsat with hb | ⟨hw, m, hm, hfam⟩
  · -- b itself is proven
    split
    · rename_i hwin
      cases ha : (univ.filter (fun m => FLit.sameFamily m b)).all
          (deadLit lambda N)
      · rfl
      · exfalso
        have hbmem : b ∈ univ.filter (fun m => FLit.sameFamily m b) := by
          rw [List.mem_filter]
          refine ⟨hPu b hb, ?_⟩
          simp [FLit.sameFamily]
        have := List.all_eq_true.mp ha b hbmem
        rw [not_dead b hb] at this
        exact Bool.noConfusion this
    · exact not_dead b hb
  · -- a family member is proven; b is atemporal
    rw [if_pos (by rw [hw]; rfl)]
    cases ha : (univ.filter (fun m' => FLit.sameFamily m' b)).all
        (deadLit lambda N)
    · rfl
    · exfalso
      have hmmem : m ∈ univ.filter (fun m' => FLit.sameFamily m' b) := by
        rw [List.mem_filter]
        exact ⟨hPu m hm, hfam⟩
      have := List.all_eq_true.mp ha m hmmem
      rw [not_dead m hm] at this
      exact Bool.noConfusion this


/-! ## Consistency of the two-sided model -/

/-- The superiority relation on family rules. -/
def FSupRel (t : FTheory) (a b : FRule) : Prop :=
  t.isSuperior a.label b.label = true

theorem fSupRel_wellFounded_of_no_superiority (t : FTheory)
    (hsup : t.superiority = []) : WellFounded (FSupRel t) := by
  constructor
  intro a
  constructor
  intro y hy
  simp only [FSupRel, FTheory.isSuperior, hsup, List.any_nil] at hy
  exact absurd hy Bool.false_ne_true

theorem gatedDeltaF_subset (delta : List FLit) :
    ∀ l ∈ gatedDeltaF delta, l ∈ delta :=
  fun _l hl => (List.mem_filter.mp hl).1

/-- **Two-sided consistency (core)**: for any theory with well-founded
    superiority, and any delta/lambda/universe satisfying the
    pipeline containments, the two-sided proven set never contains a
    complementary pair. Proof: both gates kill the subsumption branches,
    so both literals hold via supporters; each supporter attacks the other
    literal and must be fact-exempt (contradicts a gate), discarded
    (contradicts its own satisfied body, which is alive by the P-invariants),
    or team-defeated — yielding an infinite ascending superiority chain,
    impossible under well-foundedness.

    Fact-rule well-formedness (`fact → body = []`) is NOT required: the only
    fact-related premise the proof consumes is `hfactd`, which places every
    fact head in `delta`. -/
theorem twoSidedClose_consistent (t : FTheory)
    (hwf : WellFounded (FSupRel t))
    (delta lambda univ : List FLit)
    (hdl : ∀ x ∈ delta, x ∈ lambda)
    (hlu : ∀ x ∈ lambda, x ∈ univ)
    (hfactd : ∀ s ∈ t.rules, s.isFact = true → s.head ∈ delta)
    (fuel : Nat) (l : FLit)
    (hl : l ∈ (twoSidedClose t univ delta lambda
      (gatedDeltaF delta) [] fuel).1)
    (hcl : l.complement ∈ (twoSidedClose t univ delta lambda
      (gatedDeltaF delta) [] fuel).1) :
    False := by
  -- Soundness at the final state.
  have hsound := twoSidedClose_P_sound t univ delta lambda
    (gatedDeltaF delta) [] fuel
    (fun x hx => gatedDelta_canProve2 t univ delta lambda x hx _ _)
  have hcanl := hsound l hl
  have hcanc := hsound l.complement hcl
  -- Invariants of the final proven set.
  have hPl : ∀ x ∈ (twoSidedClose t univ delta lambda
      (gatedDeltaF delta) [] fuel).1, x ∈ lambda :=
    twoSidedClose_P_subset_lambda t univ delta lambda _ [] fuel
      (fun x hx => hdl x (gatedDeltaF_subset delta x hx))
  have hPu : ∀ x ∈ (twoSidedClose t univ delta lambda
      (gatedDeltaF delta) [] fuel).1, x ∈ univ :=
    fun x hx => hlu x (hPl x hx)
  have hPN : ∀ x ∈ (twoSidedClose t univ delta lambda
      (gatedDeltaF delta) [] fuel).1,
      x ∉ (twoSidedClose t univ delta lambda
        (gatedDeltaF delta) [] fuel).2 :=
    twoSidedClose_disjoint t univ delta lambda _ [] fuel
      (fun x _ hx => (List.not_mem_nil (a := x)).elim hx)
  -- The gates: neither literal's complement is definite.
  have hgatel : delta.contains l.complement = false := by
    cases hc : delta.contains l.complement
    · rfl
    · exfalso
      unfold canProve2 at hcanl
      rw [if_pos hc] at hcanl
      exact Bool.noConfusion hcanl
  have hgatec : delta.contains l = false := by
    cases hc : delta.contains l
    · rfl
    · exfalso
      unfold canProve2 at hcanc
      rw [FLit.complement_involutive, if_pos hc] at hcanc
      exact Bool.noConfusion hcanc
  -- Both literals hold via the rule path.
  unfold canProve2 at hcanl hcanc
  rw [if_neg (by rw [hgatel]; exact Bool.false_ne_true),
      if_neg (by rw [hgatec]; exact Bool.false_ne_true)] at hcanl
  rw [FLit.complement_involutive,
      if_neg (by rw [hgatec]; exact Bool.false_ne_true),
      if_neg (by rw [hgatel]; exact Bool.false_ne_true)] at hcanc
  rw [Bool.and_eq_true] at hcanl hcanc
  obtain ⟨hsupl, hattl⟩ := hcanl
  obtain ⟨hsupc, hattc⟩ := hcanc
  -- Key induction: no productive rule with satisfied body heads either
  -- literal.
  have key : ∀ s : FRule, s ∈ t.rules → s.isProductive = true →
      (s.head = l ∨ s.head = l.complement) →
      s.bodySat (twoSidedClose t univ delta lambda
        (gatedDeltaF delta) [] fuel).1 = true → False := by
    intro s
    induction s using hwf.induction with
    | _ s ih =>
      intro hs hprod hhead hbody
      cases hhead with
      | inl hh =>
        -- s heads l, hence attacks l.complement; use its attack condition
        -- (already normalized to rulesWithHead l by the involutive rewrite).
        have hs_att : s ∈ t.rulesWithHead l := by
          simp only [FTheory.rulesWithHead, List.mem_filter]
          exact ⟨hs, by rw [hh]; exact beq_self_eq_true _⟩
        have hdisj := List.all_eq_true.mp hattc s hs_att
        rw [Bool.or_eq_true, Bool.or_eq_true] at hdisj
        rcases hdisj with (hfacts | hdisc) | hteam
        · have hd := hfactd s hs hfacts
          rw [hh] at hd
          have hc := List.contains_iff_mem.mpr hd
          rw [hgatec] at hc
          exact Bool.noConfusion hc
        · simp only [discardedRule, List.any_eq_true] at hdisc
          obtain ⟨b, hb, hbd⟩ := hdisc
          have hbsat : FLit.famSat (twoSidedClose t univ delta lambda
              (gatedDeltaF delta) [] fuel).1 b = true := by
            have hall := hbody
            simp only [FRule.bodySat, List.all_eq_true] at hall
            exact hall b hb
          rw [not_bodyDead_of_famSat univ lambda _ _ hPl hPu hPN b hbsat]
            at hbd
          exact Bool.noConfusion hbd
        · simp only [teamDefeats2, List.any_eq_true] at hteam
          obtain ⟨d, hdm, hdc⟩ := hteam
          simp only [Bool.and_eq_true] at hdc
          obtain ⟨⟨hdprod, hdbody⟩, hdsup⟩ := hdc
          simp only [FTheory.rulesWithHead, List.mem_filter,
            beq_iff_eq] at hdm
          exact ih d hdsup hdm.1 hdprod (Or.inr hdm.2) hdbody
      | inr hh =>
        -- s heads l.complement, hence attacks l; use l's attack condition.
        have hs_att : s ∈ t.rulesWithHead l.complement := by
          simp only [FTheory.rulesWithHead, List.mem_filter]
          exact ⟨hs, by rw [hh]; exact beq_self_eq_true _⟩
        have hdisj := List.all_eq_true.mp hattl s hs_att
        rw [Bool.or_eq_true, Bool.or_eq_true] at hdisj
        rcases hdisj with (hfacts | hdisc) | hteam
        · have hd := hfactd s hs hfacts
          rw [hh] at hd
          have hc := List.contains_iff_mem.mpr hd
          rw [hgatel] at hc
          exact Bool.noConfusion hc
        · simp only [discardedRule, List.any_eq_true] at hdisc
          obtain ⟨b, hb, hbd⟩ := hdisc
          have hbsat : FLit.famSat (twoSidedClose t univ delta lambda
              (gatedDeltaF delta) [] fuel).1 b = true := by
            have hall := hbody
            simp only [FRule.bodySat, List.all_eq_true] at hall
            exact hall b hb
          rw [not_bodyDead_of_famSat univ lambda _ _ hPl hPu hPN b hbsat]
            at hbd
          exact Bool.noConfusion hbd
        · simp only [teamDefeats2, List.any_eq_true] at hteam
          obtain ⟨d, hdm, hdc⟩ := hteam
          simp only [Bool.and_eq_true] at hdc
          obtain ⟨⟨hdprod, hdbody⟩, hdsup⟩ := hdc
          simp only [FTheory.rulesWithHead, List.mem_filter,
            beq_iff_eq] at hdm
          exact ih d hdsup hdm.1 hdprod (Or.inl hdm.2) hdbody
  -- Apply the key to l's supporter.
  simp only [List.any_eq_true] at hsupl
  obtain ⟨rl, hrl, hrlcond⟩ := hsupl
  rw [Bool.and_eq_true] at hrlcond
  simp only [FTheory.rulesWithHead, List.mem_filter, beq_iff_eq] at hrl
  exact key rl hrl.1 hrlcond.1 (Or.inl hrl.2) hrlcond.2

/-- **Two-sided consistency**: the proven set of `famReason2` never
    contains a complementary pair — for every theory whose superiority
    relation is well-founded, under arbitrarily contradictory input.
    Extends `partial_consistent` from the three-phase model to the
    operational two-sided reference.

    Well-formedness of fact rules is not needed; see `twoSidedClose_consistent`. -/
theorem twoSided_consistent (t : FTheory)
    (hwf : WellFounded (FSupRel t)) (l : FLit)
    (hl : l ∈ (famReason2 t).2)
    (hcl : l.complement ∈ (famReason2 t).2) : False := by
  refine twoSidedClose_consistent t hwf
    (deltaCloseWith FRule.bodySat t)
    (lambdaCloseWith FRule.bodySat t (deltaCloseWith FRule.bodySat t))
    t.allLiterals
    (famDelta_subset_lambda t _)
    (famLambda_subset_univ t _ (famDelta_subset_univ t))
    (fun s hs hf => fact_head_mem_famDelta t s hs hf)
    (2 * t.allLiterals.length + 2) l hl hcl

end Family
