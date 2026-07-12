/-
  SpindleLean.Properties.Faithfulness
  Faithfulness to DL(d) paper semantics.
-/
import SpindleLean.Reason
import SpindleLean.Properties.Subset
import SpindleLean.Properties.Soundness
import SpindleLean.Properties.Equivalence
import SpindleLean.Properties.Util
import Mathlib.Data.List.Dedup
import Mathlib.Data.Finset.Dedup
import Mathlib.Data.Finset.Card

namespace Properties

def PaperDefiniteProvable (t : Theory) (delta : List Literal) (l : Literal) : Prop :=
  ∃ r ∈ t.rules, r.isDefinite = true ∧ r.head = l ∧ r.bodySatisfied delta = true

/-- The spec's +d condition (DEFEASIBLE-LOGIC-SEMANTICS.md): the
    complement-not-definite gate (condition (2)) applies to BOTH the
    delta-subsumption clause and the rule clause. When +D l and +D ~l
    both hold, neither is defeasibly provable (worked example 1). -/
def PaperDefeasibleProvable (t : Theory) (delta lambda partial_ : List Literal)
    (l : Literal) : Prop :=
  ¬ (delta.contains l.complement = true)
  ∧ (l ∈ delta
     ∨ ((∃ r ∈ t.rules, r.isProductive = true ∧ r.head = l ∧ r.bodySatisfied partial_ = true)
        ∧ Closure.allAttacksDefeated t l lambda partial_ = true))

private theorem deltaClose_go_nodup (t : Theory) (current : List Literal) (fuel : Nat)
    (hnodup : current.Nodup) : (Closure.deltaClose.go t current fuel).Nodup := by
  induction fuel generalizing current with
  | zero => simp only [Closure.deltaClose.go]; exact hnodup
  | succ n ih =>
    simp only [Closure.deltaClose.go]
    split
    · exact hnodup
    · exact ih _ (by simp only [Closure.deltaStep]; exact List.nodup_dedup _)

private theorem deltaClose_nodup (t : Theory) : (Closure.deltaClose t).Nodup := by
  simp only [Closure.deltaClose]
  exact deltaClose_go_nodup t _ _ (List.nodup_dedup _)

private theorem delta_go_fixpoint_eq (t : Theory) (current : List Literal) (n : Nat)
    (hlen : ((Closure.deltaStep t current).length == current.length) = true) :
    Closure.deltaClose.go t current (n + 1) = current := by
  simp only [Closure.deltaClose.go, hlen, ite_true]

private theorem delta_go_step_eq (t : Theory) (current : List Literal) (n : Nat)
    (hlen : ¬ ((Closure.deltaStep t current).length == current.length) = true) :
    Closure.deltaClose.go t current (n + 1) = Closure.deltaClose.go t (Closure.deltaStep t current) n := by
  simp only [Closure.deltaClose.go]; rw [if_neg hlen]

private theorem lambda_go_fixpoint_eq (t : Theory) (delta current : List Literal) (n : Nat)
    (hlen : ((Closure.lambdaStep t delta current).length == current.length) = true) :
    Closure.lambdaClose.go t delta current (n + 1) = current := by
  simp only [Closure.lambdaClose.go, hlen, ite_true]

private theorem lambda_go_step_eq (t : Theory) (delta current : List Literal) (n : Nat)
    (hlen : ¬ ((Closure.lambdaStep t delta current).length == current.length) = true) :
    Closure.lambdaClose.go t delta current (n + 1) = Closure.lambdaClose.go t delta (Closure.lambdaStep t delta current) n := by
  simp only [Closure.lambdaClose.go]; rw [if_neg hlen]

private theorem mem_lambdaStep_of_productive (t : Theory) (delta current : List Literal)
    (r : Rule) (hr : r ∈ t.rules) (hprod : r.isProductive = true)
    (hbody : r.bodySatisfied current = true)
    (hnotmem : ¬ current.contains r.head = true)
    (hnotcomp : ¬ delta.contains r.head.complement = true) :
    r.head ∈ Closure.lambdaStep t delta current := by
  simp only [Closure.lambdaStep, List.mem_dedup, List.mem_append]
  right; simp only [List.mem_filterMap]
  refine ⟨r, hr, ?_⟩
  have hnotmem_bool : current.contains r.head = false := by
    cases hc : current.contains r.head
    · rfl
    · exact absurd hc hnotmem
  have hnotcomp_bool : delta.contains r.head.complement = false := by
    cases hc : delta.contains r.head.complement
    · rfl
    · exact absurd hc hnotcomp
  simp only [hprod, hbody, hnotmem_bool, hnotcomp_bool, Bool.and_self,
    Bool.not_false, ite_true]

-- At a lambda fixpoint, if the rule conditions hold, head must be in the set
private theorem lambda_fixpoint_mem (t : Theory) (delta current : List Literal)
    (hnodup : current.Nodup)
    (hfp : ((Closure.lambdaStep t delta current).length == current.length) = true)
    (r : Rule) (hr : r ∈ t.rules) (hprod : r.isProductive = true)
    (hbody : r.bodySatisfied current = true)
    (hnotcomp : ¬ delta.contains r.head.complement = true) :
    r.head ∈ current := by
  by_contra h_not_mem
  have hnotcontains : ¬ current.contains r.head = true := fun hc =>
    h_not_mem (List.contains_iff_mem.mp hc)
  have hnotcomp_bool : delta.contains r.head.complement = false := by
    cases hc : delta.contains r.head.complement
    · rfl
    · exact absurd hc hnotcomp
  have hnotcontains_bool : current.contains r.head = false := by
    cases hc : current.contains r.head
    · rfl
    · exact absurd hc hnotcontains
  have h_in_step : r.head ∈ Closure.lambdaStep t delta current := by
    simp only [Closure.lambdaStep, List.mem_dedup, List.mem_append]
    right; simp only [List.mem_filterMap]
    refine ⟨r, hr, ?_⟩
    simp only [hprod, hbody, hnotcontains_bool, hnotcomp_bool, Bool.and_self,
      Bool.not_false, ite_true]
  exact nodup_subset_length_absurd hnodup
    (by simp only [Closure.lambdaStep]; exact List.nodup_dedup _)
    (fun x hx => mem_lambdaStep_of_mem t delta current x hx)
    (by simp only [beq_iff_eq] at hfp; exact hfp)
    h_in_step h_not_mem

private theorem lambdaStep_sub_allLiterals (t : Theory) (delta current : List Literal)
    (hsub : ∀ x ∈ current, x ∈ t.allLiterals) :
    ∀ x ∈ Closure.lambdaStep t delta current, x ∈ t.allLiterals := by
  intro x hx
  simp only [Closure.lambdaStep, List.mem_dedup, List.mem_append] at hx
  cases hx with
  | inl h => exact hsub x h
  | inr h =>
    simp only [List.mem_filterMap] at h
    obtain ⟨r, hrmem, hcond⟩ := h
    split at hcond
    · simp only [Option.some.injEq] at hcond; subst hcond
      exact List.subset_dedup _ (List.mem_append.mpr (Or.inl (List.mem_map.mpr ⟨r, hrmem, rfl⟩)))
    · simp at hcond

private theorem lambdaStep_fixpoint_of_full (t : Theory) (delta current : List Literal)
    (hnodup : current.Nodup)
    (hsub : ∀ x ∈ current, x ∈ t.allLiterals)
    (hfull : t.allLiterals.length ≤ current.length) :
    ((Closure.lambdaStep t delta current).length == current.length) = true := by
  rw [beq_iff_eq]
  have h_step_sub : ∀ x ∈ Closure.lambdaStep t delta current, x ∈ current := by
    intro x hx
    simp only [Closure.lambdaStep, List.mem_dedup, List.mem_append] at hx
    cases hx with
    | inl h => exact h
    | inr h =>
      simp only [List.mem_filterMap] at h
      obtain ⟨r, hrmem, hcond⟩ := h
      exfalso
      split at hcond
      · simp only [Option.some.injEq] at hcond; subst hcond
        rename_i hcond'
        simp only [Bool.and_eq_true, Bool.not_eq_true'] at hcond'
        have h_not_in : current.contains r.head = false := hcond'.1.2
        have h_head_all : r.head ∈ t.allLiterals :=
          List.subset_dedup _ (List.mem_append.mpr (Or.inl (List.mem_map.mpr ⟨r, hrmem, rfl⟩)))
        have h_all_nodup : t.allLiterals.Nodup := List.nodup_dedup _
        have h_fin_sub : current.toFinset ⊆ t.allLiterals.toFinset :=
          fun y hy => List.mem_toFinset.mpr (hsub y (List.mem_toFinset.mp hy))
        have h_card_le : t.allLiterals.toFinset.card ≤ current.toFinset.card := by
          rw [List.toFinset_card_of_nodup hnodup,
              List.toFinset_card_of_nodup h_all_nodup]
          exact hfull
        have h_eq := Finset.eq_of_subset_of_card_le h_fin_sub h_card_le
        have h_in : r.head ∈ current :=
          List.mem_toFinset.mp (h_eq ▸ List.mem_toFinset.mpr h_head_all)
        exact absurd (List.contains_iff_mem.mpr h_in) (by rw [h_not_in]; exact Bool.false_ne_true)
      · simp at hcond
  have h_step_nodup : (Closure.lambdaStep t delta current).Nodup := by
    simp only [Closure.lambdaStep]; exact List.nodup_dedup _
  have h_le : current.length ≤ (Closure.lambdaStep t delta current).length := by
    have : current.toFinset.card ≤ (Closure.lambdaStep t delta current).toFinset.card :=
      Finset.card_le_card (fun x hx =>
        List.mem_toFinset.mpr (mem_lambdaStep_of_mem t delta current x (List.mem_toFinset.mp hx)))
    rw [List.toFinset_card_of_nodup hnodup, List.toFinset_card_of_nodup h_step_nodup] at this
    exact this
  have h_ge : (Closure.lambdaStep t delta current).length ≤ current.length := by
    have : (Closure.lambdaStep t delta current).toFinset.card ≤ current.toFinset.card :=
      Finset.card_le_card (fun x hx =>
        List.mem_toFinset.mpr (h_step_sub x (List.mem_toFinset.mp hx)))
    rw [List.toFinset_card_of_nodup h_step_nodup, List.toFinset_card_of_nodup hnodup] at this
    exact this
  omega

private theorem lambda_go_fixpoint (t : Theory) (delta current : List Literal)
    (fuel : Nat) (hnodup : current.Nodup)
    (hsub : ∀ x ∈ current, x ∈ t.allLiterals)
    (hfuel : t.allLiterals.length - current.length ≤ fuel)
    (r : Rule) (hr : r ∈ t.rules) (hprod : r.isProductive = true)
    (hbody : r.bodySatisfied (Closure.lambdaClose.go t delta current fuel) = true)
    (hnotcomp : ¬ delta.contains r.head.complement = true) :
    r.head ∈ Closure.lambdaClose.go t delta current fuel := by
  induction fuel generalizing current with
  | zero =>
    simp only [Closure.lambdaClose.go] at *
    -- hfuel : allLiterals.length ≤ current.length, so current covers allLiterals
    -- current is a fixpoint of lambdaStep
    have hfull : t.allLiterals.length ≤ current.length := by omega
    by_contra h_not_mem
    have hfp := lambdaStep_fixpoint_of_full t delta current hnodup hsub hfull
    exact h_not_mem (lambda_fixpoint_mem t delta current hnodup hfp r hr hprod hbody hnotcomp)
  | succ n ih =>
    by_cases hlen : ((Closure.lambdaStep t delta current).length == current.length) = true
    · rw [lambda_go_fixpoint_eq t delta current n hlen] at hbody ⊢
      exact lambda_fixpoint_mem t delta current hnodup hlen r hr hprod hbody hnotcomp
    · rw [lambda_go_step_eq t delta current n hlen] at hbody ⊢
      have h_step_nodup : (Closure.lambdaStep t delta current).Nodup := by
        simp only [Closure.lambdaStep]; exact List.nodup_dedup _
      have h_step_sub := lambdaStep_sub_allLiterals t delta current hsub
      have h_strict : current.length < (Closure.lambdaStep t delta current).length := by
        simp only [beq_iff_eq] at hlen
        have h_le : current.toFinset.card ≤ (Closure.lambdaStep t delta current).toFinset.card :=
          Finset.card_le_card (fun x hx =>
            List.mem_toFinset.mpr (mem_lambdaStep_of_mem t delta current x (List.mem_toFinset.mp hx)))
        rw [List.toFinset_card_of_nodup hnodup,
            List.toFinset_card_of_nodup h_step_nodup] at h_le
        omega
      have h_fuel' : t.allLiterals.length - (Closure.lambdaStep t delta current).length ≤ n := by
        have h_step_le : (Closure.lambdaStep t delta current).length ≤ t.allLiterals.length := by
          have h_all_nodup : t.allLiterals.Nodup := List.nodup_dedup _
          have : (Closure.lambdaStep t delta current).toFinset.card ≤ t.allLiterals.toFinset.card :=
            Finset.card_le_card (fun x hx =>
              List.mem_toFinset.mpr (h_step_sub x (List.mem_toFinset.mp hx)))
          rw [List.toFinset_card_of_nodup h_step_nodup,
              List.toFinset_card_of_nodup h_all_nodup] at this
          exact this
        omega
      exact ih (Closure.lambdaStep t delta current) h_step_nodup h_step_sub h_fuel' hbody

private theorem partial_go_fixpoint_eq (t : Theory) (delta lambda current : List Literal) (n : Nat)
    (hlen : ((Closure.partialStep t delta lambda current).length == current.length) = true) :
    Closure.partialClose.go t delta lambda current (n + 1) = current := by
  simp only [Closure.partialClose.go, hlen, ite_true]

private theorem partial_go_step_eq (t : Theory) (delta lambda current : List Literal) (n : Nat)
    (hlen : ¬ ((Closure.partialStep t delta lambda current).length == current.length) = true) :
    Closure.partialClose.go t delta lambda current (n + 1) =
      Closure.partialClose.go t delta lambda (Closure.partialStep t delta lambda current) n := by
  simp only [Closure.partialClose.go]; rw [if_neg hlen]

-- +D forward
private theorem delta_go_strong (t : Theory) (current : List Literal) (l : Literal)
    (fuel : Nat)
    (hseed : ∀ x, x ∈ current →
      ∃ r ∈ t.rules, r.isDefinite = true ∧ r.head = x ∧
        r.bodySatisfied (Closure.deltaClose.go t current fuel) = true)
    (h : l ∈ Closure.deltaClose.go t current fuel) :
    ∃ r ∈ t.rules, r.isDefinite = true ∧ r.head = l ∧
      r.bodySatisfied (Closure.deltaClose.go t current fuel) = true := by
  induction fuel generalizing current with
  | zero => simp only [Closure.deltaClose.go] at *; exact hseed l h
  | succ n ih =>
    by_cases hlen : ((Closure.deltaStep t current).length == current.length) = true
    · rw [delta_go_fixpoint_eq t current n hlen] at h hseed ⊢; exact hseed l h
    · rw [delta_go_step_eq t current n hlen] at h hseed ⊢
      apply ih
      · intro x hx
        simp only [Closure.deltaStep] at hx
        rw [List.mem_dedup, List.mem_append] at hx
        cases hx with
        | inl hmem => exact hseed x hmem
        | inr hmem =>
          simp only [List.mem_filterMap] at hmem
          obtain ⟨r, hrmem, hcond⟩ := hmem
          split at hcond
          · rename_i hguard
            simp only [Option.some.injEq] at hcond
            simp only [Bool.and_eq_true, Bool.not_eq_true'] at hguard
            exact ⟨r, hrmem, hguard.1.1, hcond, bodySatisfied_mono r current _
              (fun y hy => mem_deltaClose_go_of_mem t _ y n
                (mem_deltaStep_of_mem t current y hy)) hguard.1.2⟩
          · simp at hcond
      · exact h

theorem faithful_plusD_forward (t : Theory) (l : Literal)
    (hwf : Theory.WellFormed t)
    (h : l ∈ Closure.deltaClose t) :
    PaperDefiniteProvable t (Closure.deltaClose t) l := by
  simp only [Closure.deltaClose] at h ⊢
  exact delta_go_strong t _ l _
    (fun x hx => by
      rw [List.mem_dedup] at hx
      obtain ⟨r, hr, hrhead⟩ := List.mem_map.mp hx
      simp only [Theory.facts, List.mem_filter] at hr
      refine ⟨r, hr.1, ?_, hrhead, ?_⟩
      · simp only [Rule.isDefinite, hr.2, Bool.true_or]
      · -- Fact rule: body = [] by well-formedness, so bodySatisfied is trivially true.
        have hfact : r.ruleType = .fact := by
          have h2 := hr.2
          match r.ruleType, h2 with
          | .fact, _ => rfl
          | .strict, h => exact absurd h (by decide)
          | .defeasible, h => exact absurd h (by decide)
          | .defeater, h => exact absurd h (by decide)
        have hbody_empty : r.body = [] := hwf r hr.1 hfact
        rw [Rule.bodySatisfied, hbody_empty]; rfl) h

-- +D backward
-- At a fixpoint, if body satisfied then head must be in the set
private theorem delta_fixpoint_mem (t : Theory) (current : List Literal)
    (hnodup : current.Nodup)
    (hfp : ((Closure.deltaStep t current).length == current.length) = true)
    (r : Rule) (hr : r ∈ t.rules) (hdef : r.isDefinite = true)
    (hbody : r.bodySatisfied current = true) :
    r.head ∈ current := by
  by_contra h_not_mem
  have h_in_step : r.head ∈ Closure.deltaStep t current := by
    simp only [Closure.deltaStep, List.mem_dedup, List.mem_append]
    right; simp only [List.mem_filterMap]
    exact ⟨r, hr, by simp [hdef, hbody, h_not_mem]⟩
  exact nodup_subset_length_absurd hnodup
    (by simp only [Closure.deltaStep]; exact List.nodup_dedup _)
    (fun x hx => mem_deltaStep_of_mem t current x hx)
    (by simp only [beq_iff_eq] at hfp; exact hfp)
    h_in_step h_not_mem

private theorem delta_go_fixpoint (t : Theory) (current : List Literal)
    (fuel : Nat) (hnodup : current.Nodup)
    (hsub : ∀ x ∈ current, x ∈ t.allLiterals)
    (hfuel : t.allLiterals.length - current.length ≤ fuel)
    (r : Rule) (hr : r ∈ t.rules) (hdef : r.isDefinite = true)
    (hbody : r.bodySatisfied (Closure.deltaClose.go t current fuel) = true) :
    r.head ∈ Closure.deltaClose.go t current fuel := by
  induction fuel generalizing current with
  | zero =>
    simp only [Closure.deltaClose.go] at *
    have hfull : t.allLiterals.length ≤ current.length := by omega
    by_contra h_not_mem
    have hfp := deltaStep_fixpoint_of_full t current hnodup hsub hfull
    exact h_not_mem (delta_fixpoint_mem t current hnodup hfp r hr hdef hbody)
  | succ n ih =>
    by_cases hlen : ((Closure.deltaStep t current).length == current.length) = true
    · rw [delta_go_fixpoint_eq t current n hlen] at hbody ⊢
      exact delta_fixpoint_mem t current hnodup hlen r hr hdef hbody
    · rw [delta_go_step_eq t current n hlen] at hbody ⊢
      have h_step_nodup : (Closure.deltaStep t current).Nodup := by
        simp only [Closure.deltaStep]; exact List.nodup_dedup _
      have h_step_sub := deltaStep_sub_allLiterals t current hsub
      have h_strict : current.length < (Closure.deltaStep t current).length := by
        simp only [beq_iff_eq] at hlen
        have h_le : current.toFinset.card ≤ (Closure.deltaStep t current).toFinset.card :=
          Finset.card_le_card (fun x hx =>
            List.mem_toFinset.mpr (mem_deltaStep_of_mem t current x (List.mem_toFinset.mp hx)))
        rw [List.toFinset_card_of_nodup hnodup,
            List.toFinset_card_of_nodup h_step_nodup] at h_le
        omega
      have h_fuel' : t.allLiterals.length - (Closure.deltaStep t current).length ≤ n := by
        have h_step_le : (Closure.deltaStep t current).length ≤ t.allLiterals.length := by
          have h_all_nodup : t.allLiterals.Nodup := List.nodup_dedup _
          have : (Closure.deltaStep t current).toFinset.card ≤ t.allLiterals.toFinset.card :=
            Finset.card_le_card (fun x hx =>
              List.mem_toFinset.mpr (h_step_sub x (List.mem_toFinset.mp hx)))
          rw [List.toFinset_card_of_nodup h_step_nodup,
              List.toFinset_card_of_nodup h_all_nodup] at this
          exact this
        omega
      exact ih (Closure.deltaStep t current) h_step_nodup h_step_sub h_fuel' hbody

private theorem facts_dedup_sub_allLiterals (t : Theory) :
    ∀ x ∈ (t.facts.map (·.head)).dedup, x ∈ t.allLiterals := by
  intro x hx
  rw [List.mem_dedup] at hx
  simp only [List.mem_map, Theory.facts, List.mem_filter] at hx
  obtain ⟨r, ⟨hrmem, _⟩, hrhead⟩ := hx
  subst hrhead
  exact List.subset_dedup _ (List.mem_append.mpr (Or.inl (List.mem_map.mpr ⟨r, hrmem, rfl⟩)))

theorem faithful_plusD_backward (t : Theory) (l : Literal)
    (h : PaperDefiniteProvable t (Closure.deltaClose t) l) :
    l ∈ Closure.deltaClose t := by
  obtain ⟨r, hr, hdef, hhead, hbody⟩ := h
  rw [← hhead]; simp only [Closure.deltaClose]
  exact delta_go_fixpoint t _ (t.allLiterals.length + 1) (List.nodup_dedup _)
    (facts_dedup_sub_allLiterals t) (by omega) r hr hdef hbody

private theorem teamDefeats_mono (t : Theory) (lit : Literal) (attacker : Rule)
    (s1 s2 : List Literal) (hsub : ∀ x, x ∈ s1 → x ∈ s2)
    (h : Closure.teamDefeats t lit attacker s1 = true) :
    Closure.teamDefeats t lit attacker s2 = true := by
  simp only [Closure.teamDefeats, List.any_eq_true] at h ⊢
  obtain ⟨defender, hdef, hcond⟩ := h
  refine ⟨defender, hdef, ?_⟩
  simp only [Bool.and_eq_true] at hcond ⊢
  exact ⟨⟨hcond.1.1, bodySatisfied_mono defender s1 s2 hsub hcond.1.2⟩, hcond.2⟩

private theorem allAttacksDefeated_mono (t : Theory) (lit : Literal) (lambda : List Literal)
    (s1 s2 : List Literal) (hsub : ∀ x, x ∈ s1 → x ∈ s2)
    (h : Closure.allAttacksDefeated t lit lambda s1 = true) :
    Closure.allAttacksDefeated t lit lambda s2 = true := by
  simp only [Closure.allAttacksDefeated, List.all_eq_true] at h ⊢
  intro attacker hatt
  have h1 := h attacker hatt
  -- h1 : (attacker.isFact || !attackReaches lambda attacker || teamDefeats ...) = true
  -- The first two disjuncts don't depend on partial, the third is monotone
  cases hf : attacker.isFact
  · simp only [hf, Bool.false_or] at h1 ⊢
    cases ha : Closure.attackReaches lambda attacker
    · -- attackReaches = false, so !false = true, disjunction is trivially true
      simp only [Bool.not_false, Bool.true_or]
    · -- attackReaches = true, so !true = false, need teamDefeats
      simp only [ha, Bool.not_true, Bool.false_or] at h1 ⊢
      exact teamDefeats_mono t lit attacker s1 s2 hsub h1
  · simp only [Bool.true_or]

-- +d forward
private theorem partial_go_sound (t : Theory) (delta lambda current : List Literal)
    (l : Literal) (fuel : Nat)
    (hseed : ∀ x, x ∈ current →
      Closure.canProve t x delta lambda
        (Closure.partialClose.go t delta lambda current fuel) = true)
    (h : l ∈ Closure.partialClose.go t delta lambda current fuel) :
    Closure.canProve t l delta lambda
      (Closure.partialClose.go t delta lambda current fuel) = true := by
  induction fuel generalizing current with
  | zero => simp only [Closure.partialClose.go] at *; exact hseed l h
  | succ n ih =>
    by_cases hlen : ((Closure.partialStep t delta lambda current).length == current.length) = true
    · rw [partial_go_fixpoint_eq t delta lambda current n hlen] at h hseed ⊢; exact hseed l h
    · rw [partial_go_step_eq t delta lambda current n hlen] at h hseed ⊢
      apply ih
      · intro x hx
        simp only [Closure.partialStep] at hx
        rw [List.mem_dedup, List.mem_append] at hx
        cases hx with
        | inl hmem => exact hseed x hmem
        | inr hmem =>
          simp only [List.mem_filter, Bool.and_eq_true] at hmem
          obtain ⟨_, _, hcan⟩ := hmem
          simp only [Closure.canProve] at hcan ⊢
          -- Gate first: complement in delta makes canProve false (absurd hcan).
          cases hdc : delta.contains x.complement <;>
            simp only [hdc, Bool.false_eq_true, ite_false, ite_true] at hcan ⊢
          -- Only the complement = false branch survives.
          cases hdx : delta.contains x <;>
            simp only [hdx, Bool.false_eq_true, ite_false, ite_true] at hcan ⊢
          -- Only the x ∉ delta branch survives; monotonicity as before.
          simp only [Bool.and_eq_true] at hcan ⊢
          exact ⟨by
            simp only [List.any_eq_true] at hcan ⊢
            obtain ⟨r, hr, hp⟩ := hcan.1
            simp only [Bool.and_eq_true] at hp ⊢
            exact ⟨r, hr, hp.1, bodySatisfied_mono r current _
              (fun y hy => mem_partialClose_go_of_mem t delta lambda _ y n
                (mem_partialStep_of_mem t delta lambda current y hy)) hp.2⟩,
          by
            exact allAttacksDefeated_mono t x lambda current _
              (fun y hy => mem_partialClose_go_of_mem t delta lambda _ y n
                (mem_partialStep_of_mem t delta lambda current y hy)) hcan.2⟩
      · exact h

theorem faithful_plusd_forward (t : Theory) (l : Literal)
    (h : l ∈ Closure.partialClose t (Closure.deltaClose t)
            (Closure.lambdaClose t (Closure.deltaClose t))) :
    PaperDefeasibleProvable t (Closure.deltaClose t)
      (Closure.lambdaClose t (Closure.deltaClose t))
      (Closure.partialClose t (Closure.deltaClose t)
        (Closure.lambdaClose t (Closure.deltaClose t))) l := by
  set delta := Closure.deltaClose t; set lambda := Closure.lambdaClose t delta
  set partial_ := Closure.partialClose t delta lambda
  have hcan : Closure.canProve t l delta lambda partial_ = true := by
    simp only [Closure.partialClose, partial_] at h ⊢
    exact partial_go_sound t delta lambda (Closure.gatedDelta delta) l _
      (fun x hx => by
        have hxd := (List.mem_filter.mp hx).1
        have hxc : delta.contains x.complement = false := by
          have := (List.mem_filter.mp hx).2
          simpa only [Bool.not_eq_true'] using this
        simp only [Closure.canProve]
        rw [if_neg (by rw [hxc]; exact Bool.false_ne_true),
            if_pos (List.contains_iff_mem.mpr hxd)]) h
  simp only [Closure.canProve] at hcan
  split at hcan
  · exact absurd hcan Bool.false_ne_true
  · split at hcan
    · rename_i hnc hd
      exact ⟨hnc, Or.inl (List.contains_iff_mem.mp hd)⟩
    · rename_i hnc hnd
      refine ⟨hnc, Or.inr ?_⟩
      simp only [Bool.and_eq_true] at hcan
      refine ⟨?_, hcan.2⟩
      simp only [List.any_eq_true] at hcan
      obtain ⟨r, hr, hp⟩ := hcan.1
      simp only [Bool.and_eq_true] at hp
      simp only [Theory.rulesWithHead, List.mem_filter, beq_iff_eq] at hr
      exact ⟨r, hr.1, hp.1, hr.2, hp.2⟩

-- At a partial fixpoint, if canProve and in lambda, then must be in current
private theorem partial_fixpoint_mem (t : Theory) (delta lambda current : List Literal)
    (hnodup : current.Nodup)
    (hfp : ((Closure.partialStep t delta lambda current).length == current.length) = true)
    (l : Literal) (hlam : l ∈ lambda)
    (hcan : Closure.canProve t l delta lambda current = true) :
    l ∈ current := by
  by_contra h_not_mem
  have hnotcontains : current.contains l = false := by
    cases hc : current.contains l
    · rfl
    · exact absurd (List.contains_iff_mem.mp hc) h_not_mem
  have h_in_step : l ∈ Closure.partialStep t delta lambda current := by
    simp only [Closure.partialStep, List.mem_dedup, List.mem_append]
    right; simp only [List.mem_filter]
    refine ⟨hlam, ?_⟩
    simp only [Bool.and_eq_true, Bool.not_eq_true']
    exact ⟨hnotcontains, hcan⟩
  exact nodup_subset_length_absurd hnodup
    (by simp only [Closure.partialStep]; exact List.nodup_dedup _)
    (fun x hx => mem_partialStep_of_mem t delta lambda current x hx)
    (by simp only [beq_iff_eq] at hfp; exact hfp)
    h_in_step h_not_mem

private theorem partialStep_sub_allLiterals (t : Theory) (delta lambda current : List Literal)
    (hsub_cur : ∀ x ∈ current, x ∈ t.allLiterals)
    (hsub_lam : ∀ x ∈ lambda, x ∈ t.allLiterals) :
    ∀ x ∈ Closure.partialStep t delta lambda current, x ∈ t.allLiterals := by
  intro x hx
  simp only [Closure.partialStep, List.mem_dedup, List.mem_append] at hx
  cases hx with
  | inl h => exact hsub_cur x h
  | inr h =>
    simp only [List.mem_filter] at h
    exact hsub_lam x h.1

private theorem partialStep_fixpoint_of_full (t : Theory) (delta lambda current : List Literal)
    (hnodup : current.Nodup)
    (hsub_cur : ∀ x ∈ current, x ∈ t.allLiterals)
    (hsub_lam : ∀ x ∈ lambda, x ∈ t.allLiterals)
    (hfull : t.allLiterals.length ≤ current.length) :
    ((Closure.partialStep t delta lambda current).length == current.length) = true := by
  rw [beq_iff_eq]
  have h_step_sub : ∀ x ∈ Closure.partialStep t delta lambda current, x ∈ current := by
    intro x hx
    simp only [Closure.partialStep, List.mem_dedup, List.mem_append] at hx
    cases hx with
    | inl h => exact h
    | inr h =>
      simp only [List.mem_filter, Bool.and_eq_true, Bool.not_eq_true'] at h
      -- x ∈ lambda, !current.contains x, canProve x ...
      -- x ∈ lambda ⊆ allLiterals. current covers allLiterals (by fullness). So x ∈ current.
      -- But h says !current.contains x. Contradiction.
      exfalso
      have h_in_all := hsub_lam x h.1
      have h_all_nodup : t.allLiterals.Nodup := List.nodup_dedup _
      have h_fin_sub : current.toFinset ⊆ t.allLiterals.toFinset :=
        fun y hy => List.mem_toFinset.mpr (hsub_cur y (List.mem_toFinset.mp hy))
      have h_card_le : t.allLiterals.toFinset.card ≤ current.toFinset.card := by
        rw [List.toFinset_card_of_nodup hnodup,
            List.toFinset_card_of_nodup h_all_nodup]
        exact hfull
      have h_eq := Finset.eq_of_subset_of_card_le h_fin_sub h_card_le
      have h_in : x ∈ current :=
        List.mem_toFinset.mp (h_eq ▸ List.mem_toFinset.mpr h_in_all)
      exact absurd (List.contains_iff_mem.mpr h_in) (by rw [h.2.1]; exact Bool.false_ne_true)
  have h_step_nodup : (Closure.partialStep t delta lambda current).Nodup := by
    simp only [Closure.partialStep]; exact List.nodup_dedup _
  have h_le : current.length ≤ (Closure.partialStep t delta lambda current).length := by
    have : current.toFinset.card ≤ (Closure.partialStep t delta lambda current).toFinset.card :=
      Finset.card_le_card (fun x hx =>
        List.mem_toFinset.mpr (mem_partialStep_of_mem t delta lambda current x (List.mem_toFinset.mp hx)))
    rw [List.toFinset_card_of_nodup hnodup, List.toFinset_card_of_nodup h_step_nodup] at this
    exact this
  have h_ge : (Closure.partialStep t delta lambda current).length ≤ current.length := by
    have : (Closure.partialStep t delta lambda current).toFinset.card ≤ current.toFinset.card :=
      Finset.card_le_card (fun x hx =>
        List.mem_toFinset.mpr (h_step_sub x (List.mem_toFinset.mp hx)))
    rw [List.toFinset_card_of_nodup h_step_nodup, List.toFinset_card_of_nodup hnodup] at this
    exact this
  omega

private theorem partial_go_fixpoint (t : Theory) (delta lambda current : List Literal)
    (fuel : Nat) (hnodup : current.Nodup)
    (hsub_cur : ∀ x ∈ current, x ∈ t.allLiterals)
    (hsub_lam : ∀ x ∈ lambda, x ∈ t.allLiterals)
    (hfuel : t.allLiterals.length - current.length ≤ fuel)
    (l : Literal) (hlam : l ∈ lambda)
    (hcan : Closure.canProve t l delta lambda
      (Closure.partialClose.go t delta lambda current fuel) = true) :
    l ∈ Closure.partialClose.go t delta lambda current fuel := by
  induction fuel generalizing current with
  | zero =>
    simp only [Closure.partialClose.go] at *
    have hfull : t.allLiterals.length ≤ current.length := by omega
    by_contra h_not_mem
    have hfp := partialStep_fixpoint_of_full t delta lambda current hnodup hsub_cur hsub_lam hfull
    exact h_not_mem (partial_fixpoint_mem t delta lambda current hnodup hfp l hlam hcan)
  | succ n ih =>
    by_cases hlen : ((Closure.partialStep t delta lambda current).length == current.length) = true
    · rw [partial_go_fixpoint_eq t delta lambda current n hlen] at hcan ⊢
      exact partial_fixpoint_mem t delta lambda current hnodup hlen l hlam hcan
    · rw [partial_go_step_eq t delta lambda current n hlen] at hcan ⊢
      have h_step_nodup : (Closure.partialStep t delta lambda current).Nodup := by
        simp only [Closure.partialStep]; exact List.nodup_dedup _
      have h_step_sub := partialStep_sub_allLiterals t delta lambda current hsub_cur hsub_lam
      have h_strict : current.length < (Closure.partialStep t delta lambda current).length := by
        simp only [beq_iff_eq] at hlen
        have h_le : current.toFinset.card ≤ (Closure.partialStep t delta lambda current).toFinset.card :=
          Finset.card_le_card (fun x hx =>
            List.mem_toFinset.mpr (mem_partialStep_of_mem t delta lambda current x (List.mem_toFinset.mp hx)))
        rw [List.toFinset_card_of_nodup hnodup,
            List.toFinset_card_of_nodup h_step_nodup] at h_le
        omega
      have h_fuel' : t.allLiterals.length - (Closure.partialStep t delta lambda current).length ≤ n := by
        have h_step_le : (Closure.partialStep t delta lambda current).length ≤ t.allLiterals.length := by
          have h_all_nodup : t.allLiterals.Nodup := List.nodup_dedup _
          have : (Closure.partialStep t delta lambda current).toFinset.card ≤ t.allLiterals.toFinset.card :=
            Finset.card_le_card (fun x hx =>
              List.mem_toFinset.mpr (h_step_sub x (List.mem_toFinset.mp hx)))
          rw [List.toFinset_card_of_nodup h_step_nodup,
              List.toFinset_card_of_nodup h_all_nodup] at this
          exact this
        omega
      exact ih (Closure.partialStep t delta lambda current) h_step_nodup h_step_sub h_fuel' hcan

private theorem deltaClose_go_sub_allLiterals (t : Theory) (current : List Literal)
    (fuel : Nat) (hnodup : current.Nodup)
    (hsub : ∀ x ∈ current, x ∈ t.allLiterals) :
    ∀ x ∈ Closure.deltaClose.go t current fuel, x ∈ t.allLiterals := by
  induction fuel generalizing current with
  | zero => simp only [Closure.deltaClose.go]; exact hsub
  | succ n ih =>
    simp only [Closure.deltaClose.go]
    split
    · exact hsub
    · exact ih _ (by simp only [Closure.deltaStep]; exact List.nodup_dedup _)
        (deltaStep_sub_allLiterals t current hsub)

private theorem deltaClose_sub_allLiterals (t : Theory) :
    ∀ x ∈ Closure.deltaClose t, x ∈ t.allLiterals := by
  simp only [Closure.deltaClose]
  exact deltaClose_go_sub_allLiterals t _ (t.allLiterals.length + 1) (List.nodup_dedup _)
    (facts_dedup_sub_allLiterals t)

private theorem lambdaClose_go_sub_allLiterals (t : Theory) (delta current : List Literal)
    (fuel : Nat) (hnodup : current.Nodup)
    (hsub : ∀ x ∈ current, x ∈ t.allLiterals) :
    ∀ x ∈ Closure.lambdaClose.go t delta current fuel, x ∈ t.allLiterals := by
  induction fuel generalizing current with
  | zero => simp only [Closure.lambdaClose.go]; exact hsub
  | succ n ih =>
    simp only [Closure.lambdaClose.go]
    split
    · exact hsub
    · exact ih _ (by simp only [Closure.lambdaStep]; exact List.nodup_dedup _)
        (lambdaStep_sub_allLiterals t delta current hsub)

private theorem lambdaClose_sub_allLiterals (t : Theory) (delta : List Literal)
    (hnodup_delta : delta.Nodup)
    (hsub_delta : ∀ x ∈ delta, x ∈ t.allLiterals) :
    ∀ x ∈ Closure.lambdaClose t delta, x ∈ t.allLiterals := by
  simp only [Closure.lambdaClose]
  exact lambdaClose_go_sub_allLiterals t delta delta (t.allLiterals.length + 1)
    hnodup_delta hsub_delta

-- Helper for the backward direction to avoid heartbeat timeouts
set_option maxHeartbeats 400000 in
private theorem plusd_backward_core (t : Theory)
    (delta : List Literal) (hd : delta = Closure.deltaClose t)
    (lambda : List Literal) (hl : lambda = Closure.lambdaClose t delta)
    (partial_ : List Literal) (hp : partial_ = Closure.partialClose t delta lambda)
    (r : Rule) (hr : r ∈ t.rules) (hprod : r.isProductive = true)
    (hbody : r.bodySatisfied partial_ = true)
    (hnotcomp : ¬ delta.contains r.head.complement = true)
    (hattacks : Closure.allAttacksDefeated t r.head lambda partial_ = true) :
    r.head ∈ partial_ := by
  -- r.bodySatisfied partial_ = true, and partial_ ⊆ lambda
  have hsub : ∀ x, x ∈ partial_ → x ∈ lambda := by
    subst hd; subst hl; subst hp; exact fun x hx => partial_subset_lambda t x hx
  have hbody_lam : r.bodySatisfied lambda = true :=
    bodySatisfied_mono r _ _ hsub hbody
  -- Show r.head ∈ lambda
  have hnodup_delta : delta.Nodup := by subst hd; exact deltaClose_nodup t
  have hsub_delta : ∀ x ∈ delta, x ∈ t.allLiterals := by
    subst hd; exact deltaClose_sub_allLiterals t
  have hlam : r.head ∈ lambda := by
    subst hl
    show r.head ∈ Closure.lambdaClose.go t delta delta (t.allLiterals.length + 1)
    have hbody_go : r.bodySatisfied
        (Closure.lambdaClose.go t delta delta (t.allLiterals.length + 1)) = true := hbody_lam
    exact lambda_go_fixpoint t delta delta (t.allLiterals.length + 1)
      hnodup_delta hsub_delta (by omega) r hr hprod hbody_go hnotcomp
  -- Show canProve returns true
  have hnotcomp_bool : delta.contains r.head.complement = false := by
    cases hc : delta.contains r.head.complement
    · rfl
    · exact absurd hc hnotcomp
  have hcan : Closure.canProve t r.head delta lambda partial_ = true := by
    simp only [Closure.canProve]
    rw [if_neg (by rw [hnotcomp_bool]; exact Bool.false_ne_true)]
    by_cases hdl : delta.contains r.head = true
    · rw [if_pos hdl]
    · rw [if_neg hdl]
      simp only [Bool.and_eq_true]
      constructor
      · simp only [List.any_eq_true]
        refine ⟨r, ?_, ?_⟩
        · simp only [Theory.rulesWithHead, List.mem_filter]
          exact ⟨hr, by simp⟩
        · simp only [Bool.and_eq_true]; exact ⟨hprod, hbody⟩
      · exact hattacks
  -- Apply partial fixpoint (seed is the gated delta)
  subst hp
  have hsub_lam : ∀ x ∈ lambda, x ∈ t.allLiterals := by
    subst hl
    exact lambdaClose_sub_allLiterals t delta hnodup_delta hsub_delta
  exact partial_go_fixpoint t delta lambda (Closure.gatedDelta delta)
    (t.allLiterals.length + 1) (hnodup_delta.filter _)
    (fun x hx => hsub_delta x (gatedDelta_subset delta x hx))
    hsub_lam (by omega) r.head hlam hcan

-- +d backward
theorem faithful_plusd_backward (t : Theory) (l : Literal)
    (h : PaperDefeasibleProvable t (Closure.deltaClose t)
          (Closure.lambdaClose t (Closure.deltaClose t))
          (Closure.partialClose t (Closure.deltaClose t)
            (Closure.lambdaClose t (Closure.deltaClose t))) l) :
    l ∈ Closure.partialClose t (Closure.deltaClose t)
          (Closure.lambdaClose t (Closure.deltaClose t)) := by
  obtain ⟨hnotcomp, hrest⟩ := h
  cases hrest with
  | inl hdelta =>
    exact delta_subset_partial t l hdelta
      (fun hmem => hnotcomp (List.contains_iff_mem.mpr hmem))
  | inr h =>
    obtain ⟨⟨r, hr, hprod, hhead, hbody⟩, hattacks⟩ := h
    subst hhead
    exact plusd_backward_core t _ rfl _ rfl _ rfl r hr hprod hbody hnotcomp hattacks

theorem faithful_ambiguity_blocking (t : Theory) (l : Literal)
    (delta lambda partial_ : List Literal)
    (hnotD : delta.contains l = false)
    (hnotDcomp : delta.contains l.complement = false)
    (hattack : Closure.allAttacksDefeated t l lambda partial_ = false)
    (hattack_comp : Closure.allAttacksDefeated t l.complement lambda partial_ = false) :
    Closure.canProve t l delta lambda partial_ = false
    ∧ Closure.canProve t l.complement delta lambda partial_ = false :=
  ambiguity_blocks_both t l delta lambda partial_ hnotD hnotDcomp hattack hattack_comp

/-- +D l implies +d l, PROVIDED the definite level is consistent for l
    (~l is not also definite). The consistency hypothesis is required:
    when +D l and +D ~l both hold, the engine deliberately withholds +d
    from both — spec condition (2), worked example 1. -/
theorem faithful_D_implies_d (t : Theory) (l : Literal)
    (h : l ∈ Closure.deltaClose t)
    (hcons : l.complement ∉ Closure.deltaClose t) :
    l ∈ Closure.partialClose t (Closure.deltaClose t)
          (Closure.lambdaClose t (Closure.deltaClose t)) :=
  delta_subset_partial t l h hcons

end Properties
