/-
  SpindleLean.Properties.Consistency
  Consistency of the defeasible level: `partial` never contains a
  complementary pair.

  This is the payoff of gating +D → +d subsumption on delta-consistency
  (spec condition (2), DEFEASIBLE-LOGIC-SEMANTICS.md worked example 1):
  unlike standard Antoniou et al. defeasible logic — where an inconsistent
  strict part propagates +∂p and +∂~p — the engine quarantines strict
  contradictions, so the defeasible level is consistent for EVERY input
  theory whose superiority relation is well-founded (in particular, every
  finite acyclic superiority relation, and trivially every theory with no
  superiority declarations at all).

  Proof sketch: suppose l and ~l are both in partial. By faithfulness each
  satisfies the paper's +d condition. The delta-subsumption cases contradict
  each other's gates immediately. In the rule cases, each side's supporter
  is an applicable attacker of the other side, so it must be defeated —
  facts land in delta (contradicting a gate), applicable attackers always
  reach lambda, so team defeat must apply: a strictly superior applicable
  defender exists. That defender is again an applicable attacker of the
  opposite side, yielding an infinite ascending superiority chain — killed
  by well-founded induction on the superiority relation.
-/
import SpindleLean.Properties.Faithfulness

namespace Properties

/-- The superiority relation viewed as a relation on rules:
    `SupRel t a b` iff rule `a` is declared superior to rule `b`. -/
def SupRel (t : Theory) (a b : Rule) : Prop :=
  t.isSuperior a.label b.label = true

/-- A theory with no superiority declarations has a (trivially) well-founded
    superiority relation. -/
theorem supRel_wellFounded_of_no_superiority (t : Theory)
    (hsup : t.superiority = []) : WellFounded (SupRel t) := by
  constructor
  intro a
  constructor
  intro y hy
  simp only [SupRel, Theory.isSuperior, hsup, List.any_nil] at hy
  exact absurd hy Bool.false_ne_true

/-- **Defeasible-level consistency.** For any well-formed theory whose
    superiority relation is well-founded, the partial closure never
    contains both a literal and its complement.

    Hypotheses:
    - `hsize`: the standard fuel bound (theories with ≤ 1000 distinct
      literals; matches `reason_plusD_complete`)
    - `hwform`: fact rules have empty bodies (`Theory.WellFormed`)
    - `hwf`: the superiority relation is well-founded (holds for every
      acyclic relation on the finite rule set)

    This property is FALSE for standard Antoniou et al. DL, where an
    inconsistent strict part makes both `+∂ l` and `+∂ ~l` derivable. The
    delta-consistency gate (spec condition (2)) is exactly what makes it
    hold unconditionally. Note superiority cycles genuinely break
    consistency (two mutually-superior opposing rules both win), so the
    well-foundedness hypothesis is necessary, not an artifact. -/
theorem partial_consistent (t : Theory) (l : Literal)
    (hsize : t.allLiterals.length ≤ 1000)
    (hwform : t.WellFormed)
    (hwf : WellFounded (SupRel t))
    (hl : l ∈ Closure.partialClose t (Closure.deltaClose t)
            (Closure.lambdaClose t (Closure.deltaClose t)))
    (hcl : l.complement ∈ Closure.partialClose t (Closure.deltaClose t)
            (Closure.lambdaClose t (Closure.deltaClose t))) :
    False := by
  obtain ⟨hgate_l, hcase_l⟩ := faithful_plusd_forward t l hl
  obtain ⟨hgate_cl, hcase_cl⟩ := faithful_plusd_forward t l.complement hcl
  rw [Literal.complement_involutive] at hgate_cl
  -- Fact rules land in delta (needs well-formedness + completeness).
  have hfact_delta : ∀ s ∈ t.rules, s.isFact = true → s.head ∈ Closure.deltaClose t := by
    intro s hs hsf
    have hft : s.ruleType = .fact := by
      cases hrt : s.ruleType
      · rfl
      all_goals rw [Rule.isFact, hrt] at hsf; exact absurd hsf (by decide)
    have hbody : s.body = [] := hwform s hs hft
    apply reason_plusD_complete t s.head hsize
    refine ⟨s, hs, ?_, rfl, ?_⟩
    · simp only [Rule.isDefinite, hft]; decide
    · simp [Rule.bodySatisfied, hbody]
  -- Applicable rules always reach lambda (partial ⊆ lambda).
  have hreaches : ∀ s : Rule,
      s.bodySatisfied (Closure.partialClose t (Closure.deltaClose t)
        (Closure.lambdaClose t (Closure.deltaClose t))) = true →
      Closure.attackReaches (Closure.lambdaClose t (Closure.deltaClose t)) s = true := by
    intro s hbody
    simp only [Closure.attackReaches, List.all_eq_true]
    intro x hx
    have hxp := List.all_eq_true.mp hbody x hx
    exact List.contains_iff_mem.mpr
      (partial_subset_lambda t x (List.contains_iff_mem.mp hxp))
  cases hcase_l with
  | inl hdl => exact hgate_cl (List.contains_iff_mem.mpr hdl)
  | inr hrule_l =>
  cases hcase_cl with
  | inl hdcl => exact hgate_l (List.contains_iff_mem.mpr hdcl)
  | inr hrule_cl =>
  obtain ⟨⟨rl, hrl_mem, hrl_prod, hrl_head, hrl_body⟩, hatt_l⟩ := hrule_l
  obtain ⟨⟨_, _, _, _, _⟩, hatt_cl⟩ := hrule_cl
  -- Key induction: no applicable productive rule can head either literal.
  have key : ∀ s : Rule, s ∈ t.rules → s.isProductive = true →
      (s.head = l ∨ s.head = l.complement) →
      s.bodySatisfied (Closure.partialClose t (Closure.deltaClose t)
        (Closure.lambdaClose t (Closure.deltaClose t))) = true → False := by
    intro s
    induction s using hwf.induction with
    | _ s ih =>
      intro hs hprod hhead hbody
      cases hhead with
      | inl hh =>
        -- s.head = l: s attacks l.complement; use hatt_cl.
        have hs_att : s ∈ t.rulesWithHead l.complement.complement := by
          simp only [Theory.rulesWithHead, List.mem_filter, Literal.complement_involutive]
          exact ⟨hs, by rw [hh]; exact beq_self_eq_true _⟩
        have hdisj := List.all_eq_true.mp hatt_cl s hs_att
        simp only [Bool.or_eq_true] at hdisj
        rcases hdisj with (hfact | hunreach) | hteam
        · exact hgate_cl (List.contains_iff_mem.mpr (hh ▸ hfact_delta s hs hfact))
        · rw [Bool.not_eq_true'] at hunreach
          rw [hreaches s hbody] at hunreach
          exact absurd hunreach (by decide)
        · simp only [Closure.teamDefeats, List.any_eq_true] at hteam
          obtain ⟨d, hd_mem, hd_cond⟩ := hteam
          simp only [Bool.and_eq_true] at hd_cond
          obtain ⟨⟨hd_prod, hd_body⟩, hd_sup⟩ := hd_cond
          simp only [Theory.rulesWithHead, List.mem_filter, beq_iff_eq] at hd_mem
          exact ih d hd_sup hd_mem.1 hd_prod (Or.inr hd_mem.2) hd_body
      | inr hh =>
        -- s.head = l.complement: s attacks l; use hatt_l.
        have hs_att : s ∈ t.rulesWithHead l.complement := by
          simp only [Theory.rulesWithHead, List.mem_filter]
          exact ⟨hs, by rw [hh]; exact beq_self_eq_true _⟩
        have hdisj := List.all_eq_true.mp hatt_l s hs_att
        simp only [Bool.or_eq_true] at hdisj
        rcases hdisj with (hfact | hunreach) | hteam
        · exact hgate_l (List.contains_iff_mem.mpr (hh ▸ hfact_delta s hs hfact))
        · rw [Bool.not_eq_true'] at hunreach
          rw [hreaches s hbody] at hunreach
          exact absurd hunreach (by decide)
        · simp only [Closure.teamDefeats, List.any_eq_true] at hteam
          obtain ⟨d, hd_mem, hd_cond⟩ := hteam
          simp only [Bool.and_eq_true] at hd_cond
          obtain ⟨⟨hd_prod, hd_body⟩, hd_sup⟩ := hd_cond
          simp only [Theory.rulesWithHead, List.mem_filter, beq_iff_eq] at hd_mem
          exact ih d hd_sup hd_mem.1 hd_prod (Or.inl hd_mem.2) hd_body
  exact key rl hrl_mem hrl_prod (Or.inl hrl_head) hrl_body

/-- Consistency specialized to theories without superiority declarations. -/
theorem partial_consistent_no_superiority (t : Theory) (l : Literal)
    (hsize : t.allLiterals.length ≤ 1000)
    (hwform : t.WellFormed)
    (hsup : t.superiority = [])
    (hl : l ∈ Closure.partialClose t (Closure.deltaClose t)
            (Closure.lambdaClose t (Closure.deltaClose t)))
    (hcl : l.complement ∈ Closure.partialClose t (Closure.deltaClose t)
            (Closure.lambdaClose t (Closure.deltaClose t))) :
    False :=
  partial_consistent t l hsize hwform
    (supRel_wellFounded_of_no_superiority t hsup) hl hcl

end Properties
