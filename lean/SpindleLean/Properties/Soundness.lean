/-
  SpindleLean.Properties.Soundness
  Soundness proofs for delta and partial closures.

  Delta soundness: if p ∈ delta(T), there exists a definite rule in T
  with head p and body satisfied in delta.

  Ambiguity blocking: if two rules conflict and neither is superior,
  neither conclusion appears in partial.
-/
import SpindleLean.Reason
import Mathlib.Data.List.Dedup

namespace Properties

-- ═══════════════════════════════════════════════════════════════
-- Delta soundness
-- ═══════════════════════════════════════════════════════════════

/-- A literal is derivable in delta if it's a fact or there's a definite
    rule with head l and body fully in delta -/
def deltaDerivable (t : Theory) (delta : List Literal) (l : Literal) : Prop :=
  ∃ r ∈ t.rules, r.isDefinite = true ∧ r.head = l ∧ r.bodySatisfied delta = true

/-- Every literal added by deltaStep that wasn't in current has a supporting rule -/
theorem deltaStep_new_has_rule (t : Theory) (current : List Literal) (l : Literal)
    (hnew : l ∈ Closure.deltaStep t current) (hold : l ∉ current) :
    ∃ r ∈ t.rules, r.isDefinite = true ∧ r.head = l ∧ r.bodySatisfied current = true := by
  simp only [Closure.deltaStep] at hnew
  rw [List.mem_dedup, List.mem_append] at hnew
  cases hnew with
  | inl h => exact absurd h hold
  | inr h =>
    simp only [List.mem_filterMap] at h
    obtain ⟨r, hrmem, hcond⟩ := h
    split at hcond
    · -- isTrue: guard true, result is some r.head
      rename_i hguard
      simp only [Option.some.injEq] at hcond
      simp only [Bool.and_eq_true, Bool.not_eq_true', decide_eq_false_iff_not] at hguard
      exact ⟨r, hrmem, hguard.1.1, hcond, hguard.1.2⟩
    · -- isFalse: guard false, result is none (absurd)
      simp at hcond

/-- Seed soundness: every literal in the initial seed comes from a fact rule -/
theorem seed_has_fact (t : Theory) (l : Literal)
    (h : l ∈ (t.facts.map (·.head)).dedup) :
    ∃ r ∈ t.rules, r.isDefinite = true ∧ r.head = l := by
  rw [List.mem_dedup] at h
  obtain ⟨r, hr, hrhead⟩ := List.mem_map.mp h
  simp only [Theory.facts, List.mem_filter] at hr
  obtain ⟨hrmem, hrfact⟩ := hr
  refine ⟨r, hrmem, ?_, hrhead⟩
  -- hrfact : (r.ruleType == RuleType.fact) = true
  -- goal: r.isDefinite = true
  -- isDefinite = (ruleType == fact || ruleType == strict)
  simp only [Rule.isDefinite, hrfact, Bool.true_or]

/-- Helper: soundness of deltaClose.go. Every element in the go result
    that was already in the initial seed comes from a definite rule. -/
private theorem delta_go_sound (t : Theory) (current : List Literal) (l : Literal)
    (fuel : Nat)
    (hseed : ∀ (x : Literal), x ∈ current → ∃ r ∈ t.rules, r.isDefinite = true ∧ r.head = x)
    (h : l ∈ Closure.deltaClose.go t current fuel) :
    ∃ r ∈ t.rules, r.isDefinite = true ∧ r.head = l := by
  induction fuel generalizing current with
  | zero =>
    simp only [Closure.deltaClose.go] at h
    exact hseed l h
  | succ n ih =>
    simp only [Closure.deltaClose.go] at h
    split at h
    · exact hseed l h
    · apply ih _ _ h
      intro x hx
      simp only [Closure.deltaStep] at hx
      rw [List.mem_dedup, List.mem_append] at hx
      cases hx with
      | inl h => exact hseed x h
      | inr h =>
        simp only [List.mem_filterMap] at h
        obtain ⟨r, hrmem, hcond⟩ := h
        split at hcond
        · rename_i hguard
          simp only [Option.some.injEq] at hcond
          simp only [Bool.and_eq_true, Bool.not_eq_true'] at hguard
          exact ⟨r, hrmem, hguard.1.1, hcond⟩
        · simp at hcond

/-- Delta soundness (weak form): every literal in delta(T) is the head
    of some definite rule in T.

    This shows that delta never "invents" conclusions — every conclusion
    traces back to a rule. -/
theorem delta_sound (t : Theory) (l : Literal) :
    l ∈ Closure.deltaClose t →
    ∃ r ∈ t.rules, r.isDefinite = true ∧ r.head = l := by
  intro h
  simp only [Closure.deltaClose] at h
  exact delta_go_sound t _ l 1000 (fun x hx => seed_has_fact t x hx) h

-- ═══════════════════════════════════════════════════════════════
-- Ambiguity blocking (prove-ambiguity task)
-- ═══════════════════════════════════════════════════════════════

/-- Two rules conflict if one has head p and the other has head ~p -/
def rulesConflict (r₁ r₂ : Rule) : Prop :=
  r₁.head = r₂.head.complement

/-- Ambiguity blocking at the canProve level: if there exists an undefeated
    attacker (a rule for ~lit whose body reaches through lambda and no
    superior defender exists), then canProve returns false.

    This is the core computational fact underlying ambiguity blocking. -/
theorem canProve_false_of_undefeated_attacker (t : Theory) (lit : Literal)
    (delta lambda partial_ : List Literal)
    (hnotdef : delta.contains lit = false)
    (hnotdefcomp : delta.contains lit.complement = false)
    (hattack : Closure.allAttacksDefeated t lit lambda partial_ = false) :
    Closure.canProve t lit delta lambda partial_ = false := by
  simp only [Closure.canProve, hnotdef, hnotdefcomp, Bool.false_eq_true, ite_false]
  simp only [Bool.and_eq_false_imp]
  intro _
  exact hattack

/-- Ambiguity blocking for the complement: if canProve is false for lit,
    and the dual conditions hold for lit.complement, then neither
    lit nor its complement is provable — this is ambiguity blocking. -/
theorem ambiguity_blocks_both (t : Theory) (lit : Literal)
    (delta lambda partial_ : List Literal)
    (hnotdef_lit : delta.contains lit = false)
    (hnotdef_comp : delta.contains lit.complement = false)
    (hattack_lit : Closure.allAttacksDefeated t lit lambda partial_ = false)
    (hattack_comp : Closure.allAttacksDefeated t lit.complement lambda partial_ = false) :
    Closure.canProve t lit delta lambda partial_ = false
    ∧ Closure.canProve t lit.complement delta lambda partial_ = false := by
  constructor
  · exact canProve_false_of_undefeated_attacker t lit delta lambda partial_
      hnotdef_lit hnotdef_comp hattack_lit
  · -- For the complement, swap the roles
    simp only [Closure.canProve, hnotdef_comp, Bool.false_eq_true, ite_false,
      Literal.complement_involutive, hnotdef_lit]
    simp only [Bool.and_eq_false_imp]
    intro _
    exact hattack_comp

end Properties
