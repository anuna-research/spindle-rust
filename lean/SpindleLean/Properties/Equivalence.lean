/-
  SpindleLean.Properties.Equivalence
  Semantic equivalence of standard (reason) and scalable (reason_scalable)
  algorithms.

  Both compute the same least fixed point of the DL(d) consequence
  operator; the scalable version decomposes it into three phases.

  The key insight is that our `reason` function IS the three-phase
  decomposition (delta → lambda → partial), so equivalence is about
  showing this decomposition correctly computes the DL(d) consequence
  operator.

  Strategy:
  1. Define the DL(d) consequence operator semantically
  2. Show delta computes exactly the definite consequences
  3. Show lambda over-approximates the defeasible consequences
  4. Show partial computes exactly the defeasible consequences
  5. Therefore the three-phase decomposition = direct DL(d) semantics
-/
import SpindleLean.Reason
import SpindleLean.Properties.Subset
import SpindleLean.Properties.Soundness
import SpindleLean.Properties.Util
import Mathlib.Data.List.Dedup
import Mathlib.Data.Finset.Dedup
import Mathlib.Data.Finset.Card

namespace Properties

-- ═══════════════════════════════════════════════════════════════
-- The DL(d) consequence operator (semantic specification)
-- ═══════════════════════════════════════════════════════════════

/-- A literal is definitely derivable if there exists a definite (fact/strict)
    rule with head l and body fully satisfied in the current definite set -/
def isDefinitelyDerivable (t : Theory) (definiteSet : List Literal) (l : Literal) : Prop :=
  ∃ r ∈ t.rules, r.isDefinite = true ∧ r.head = l ∧ r.bodySatisfied definiteSet = true

/-- A literal is defeasibly derivable if:
    (a) it's already definite, OR
    (b) its complement is NOT definite, AND
        there exists a productive rule for it with body in the defeasible set, AND
        all attacks are defeated -/
def isDefeasiblyDerivable (t : Theory) (delta lambda defeasibleSet : List Literal)
    (l : Literal) : Prop :=
  l ∈ delta
  ∨ (l.complement ∉ delta
     ∧ ∃ r ∈ t.rules, r.isProductive = true ∧ r.head = l ∧ r.bodySatisfied defeasibleSet = true
     ∧ Closure.allAttacksDefeated t l lambda defeasibleSet = true)

-- ═══════════════════════════════════════════════════════════════
-- Delta computes definite consequences
-- ═══════════════════════════════════════════════════════════════

/-- Every element of delta has a supporting definite rule.
    (This is delta_sound from Soundness.lean, restated here for clarity.) -/
theorem delta_computes_definite (t : Theory) (l : Literal)
    (h : l ∈ Closure.deltaClose t) :
    ∃ r ∈ t.rules, r.isDefinite = true ∧ r.head = l :=
  delta_sound t l h

-- ═══════════════════════════════════════════════════════════════
-- Lambda over-approximates defeasible consequences
-- ═══════════════════════════════════════════════════════════════

/-- Everything in partial is in lambda.
    (This is partial_subset_lambda from Subset.lean, restated.) -/
theorem lambda_overapproximates_partial (t : Theory) (l : Literal)
    (h : l ∈ Closure.partialClose t (Closure.deltaClose t)
            (Closure.lambdaClose t (Closure.deltaClose t))) :
    l ∈ Closure.lambdaClose t (Closure.deltaClose t) :=
  partial_subset_lambda t l h

-- ═══════════════════════════════════════════════════════════════
-- Partial computes defeasible consequences
-- ═══════════════════════════════════════════════════════════════

/-- Every element added by partialStep that wasn't previously in current
    was proven by canProve, which checks the DL(d) condition -/
theorem partialStep_new_satisfies_canProve (t : Theory) (delta lambda current : List Literal)
    (l : Literal)
    (hnew : l ∈ Closure.partialStep t delta lambda current) (hold : l ∉ current) :
    Closure.canProve t l delta lambda current = true := by
  simp only [Closure.partialStep] at hnew
  rw [List.mem_dedup, List.mem_append] at hnew
  cases hnew with
  | inl h => exact absurd h hold
  | inr h =>
    simp only [List.mem_filter, Bool.and_eq_true] at h
    exact h.2.2

-- ═══════════════════════════════════════════════════════════════
-- The main equivalence theorem
-- ═══════════════════════════════════════════════════════════════

/-- The `reason` function computes exactly the three closures and derives
    conclusions from them. Since `reason` IS the three-phase decomposition,
    and there's only one implementation, equivalence is about showing the
    decomposition is faithful to the DL(d) semantics.

    Concretely:
    - +D l ↔ l ∈ delta(T) ↔ l has a definite derivation
    - +d l ↔ l ∈ partial(T) ↔ l is defeasibly derivable
    - -D l ↔ l ∉ delta(T)
    - -d l ↔ l ∉ partial(T)

    The forward directions (l ∈ delta → has definite derivation) are the
    soundness results. The backward directions (has derivation → l ∈ delta)
    are the completeness results, which require showing the iteration
    reaches the fixed point within the fuel budget.

    For now we state the forward (soundness) direction, which is fully proven,
    and the backward (completeness) direction with sorry. -/

-- Soundness of +D: if reason concludes +D l, then l has a definite derivation
theorem reason_plusD_sound (t : Theory) (l : Literal)
    (h : l ∈ (reason t).delta) :
    ∃ r ∈ t.rules, r.isDefinite = true ∧ r.head = l :=
  delta_sound t l h

/-- deltaClose.go result is Nodup. -/
private theorem deltaClose_go_nodup' (t : Theory) (current : List Literal) (fuel : Nat)
    (hnodup : current.Nodup) :
    (Closure.deltaClose.go t current fuel).Nodup := by
  induction fuel generalizing current with
  | zero => simp only [Closure.deltaClose.go]; exact hnodup
  | succ n ih =>
    simp only [Closure.deltaClose.go]
    split
    · exact hnodup
    · apply ih; simp only [Closure.deltaStep]; exact List.nodup_dedup _

/-- deltaStep preserves subset of allLiterals -/
theorem deltaStep_sub_allLiterals (t : Theory) (current : List Literal)
    (_hnodup : current.Nodup)
    (hsub : ∀ x ∈ current, x ∈ t.allLiterals) :
    ∀ x ∈ Closure.deltaStep t current, x ∈ t.allLiterals := by
  intro x hx
  simp only [Closure.deltaStep, List.mem_dedup, List.mem_append] at hx
  cases hx with
  | inl h => exact hsub x h
  | inr h =>
    simp only [List.mem_filterMap] at h
    obtain ⟨r, hrmem, hcond⟩ := h
    split at hcond
    · simp only [Option.some.injEq] at hcond; subst hcond
      exact List.subset_dedup _ (List.mem_append.mpr (Or.inl (List.mem_map.mpr ⟨r, hrmem, rfl⟩)))
    · simp at hcond

/-- When current contains all of allLiterals, deltaStep cannot add new elements,
    so the fixpoint length check passes. -/
theorem deltaStep_fixpoint_of_full (t : Theory) (current : List Literal)
    (hnodup : current.Nodup)
    (hsub : ∀ x ∈ current, x ∈ t.allLiterals)
    (hfull : t.allLiterals.length ≤ current.length) :
    ((Closure.deltaStep t current).length == current.length) = true := by
  rw [beq_iff_eq]
  -- deltaStep t current = (current ++ newLits).dedup where newLits are rule heads not in current
  -- All newLits are in allLiterals. But current already covers all of allLiterals by pigeonhole.
  -- So there are no new elements to add.
  have h_step_sub : ∀ x ∈ Closure.deltaStep t current, x ∈ current := by
    intro x hx
    simp only [Closure.deltaStep, List.mem_dedup, List.mem_append] at hx
    cases hx with
    | inl h => exact h
    | inr h =>
      simp only [List.mem_filterMap] at h
      obtain ⟨r, hrmem, hcond⟩ := h
      -- The filterMap returns `some r.head` only when the condition
      -- `r.isDefinite && r.bodySatisfied current && !current.contains r.head` is true.
      -- This means r.head ∉ current. But r.head ∈ allLiterals and current covers allLiterals.
      -- Contradiction.
      exfalso
      split at hcond
      · -- `some r.head` case: condition was true
        simp only [Option.some.injEq] at hcond; subst hcond
        rename_i hcond'
        -- hcond' : (r.isDefinite && r.bodySatisfied current && !current.contains r.head) = true
        have h_not_in : current.contains r.head = false := by
          simp only [Bool.and_eq_true] at hcond'
          simp only [Bool.not_eq_true'] at hcond'
          exact hcond'.2
        have h_head_all : r.head ∈ t.allLiterals :=
          List.subset_dedup _ (List.mem_append.mpr (Or.inl (List.mem_map.mpr ⟨r, hrmem, rfl⟩)))
        -- Show allLiterals ⊆ current by pigeonhole
        have h_all_nodup : t.allLiterals.Nodup := List.nodup_dedup _
        have h_fin_sub : current.toFinset ⊆ t.allLiterals.toFinset :=
          fun y hy => List.mem_toFinset.mpr (hsub y (List.mem_toFinset.mp hy))
        have h_card_le : t.allLiterals.toFinset.card ≤ current.toFinset.card := by
          rw [List.toFinset_card_of_nodup hnodup,
              List.toFinset_card_of_nodup h_all_nodup]
          exact hfull
        have h_eq := Finset.eq_of_subset_of_card_le h_fin_sub h_card_le
        have h_in_current : r.head ∈ current := by
          have : r.head ∈ t.allLiterals.toFinset := List.mem_toFinset.mpr h_head_all
          rw [← h_eq] at this
          exact List.mem_toFinset.mp this
        have h_contains : current.contains r.head = true := by
          rw [List.contains_eq_any_beq]
          exact List.any_eq_true.mpr ⟨r.head, h_in_current, beq_self_eq_true _⟩
        rw [h_not_in] at h_contains
        exact Bool.false_ne_true h_contains
      · simp at hcond
  -- Now: current ⊆ deltaStep (by mem_deltaStep_of_mem) and deltaStep ⊆ current
  -- Both Nodup, so same length
  have h_cur_sub : ∀ x ∈ current, x ∈ Closure.deltaStep t current :=
    fun x hx => mem_deltaStep_of_mem t current x hx
  have h_step_nodup : (Closure.deltaStep t current).Nodup := by
    simp only [Closure.deltaStep]; exact List.nodup_dedup _
  -- Equal sets with Nodup have equal lengths
  have h_le1 : current.toFinset.card ≤ (Closure.deltaStep t current).toFinset.card := by
    exact Finset.card_le_card (fun x hx =>
      List.mem_toFinset.mpr (h_cur_sub x (List.mem_toFinset.mp hx)))
  have h_le2 : (Closure.deltaStep t current).toFinset.card ≤ current.toFinset.card := by
    exact Finset.card_le_card (fun x hx =>
      List.mem_toFinset.mpr (h_step_sub x (List.mem_toFinset.mp hx)))
  rw [List.toFinset_card_of_nodup hnodup,
      List.toFinset_card_of_nodup h_step_nodup] at h_le1 h_le2
  omega

/-- deltaClose.go fixpoint completeness: if a definite rule has body satisfied
    in the result, its head is in the result. Requires sufficient fuel. -/
private theorem deltaClose_go_fixpoint' (t : Theory) (current : List Literal)
    (fuel : Nat) (hnodup : current.Nodup)
    (hsub : ∀ x ∈ current, x ∈ t.allLiterals)
    (hfuel : t.allLiterals.length - current.length ≤ fuel)
    (r : Rule) (hr : r ∈ t.rules) (hdef : r.isDefinite = true)
    (hbody : r.bodySatisfied (Closure.deltaClose.go t current fuel) = true) :
    r.head ∈ Closure.deltaClose.go t current fuel := by
  induction fuel generalizing current with
  | zero =>
    -- Fuel = 0, but hfuel says allLiterals.length ≤ current.length.
    -- So current is full, and is a fixpoint.
    simp only [Closure.deltaClose.go] at *
    -- current is full, so deltaStep doesn't change it => the fixpoint branch
    -- would be taken. But at fuel=0, go returns current directly.
    -- We need: r.head ∈ current. Since body is satisfied and r is definite,
    -- r.head ∈ deltaStep t current. But deltaStep t current ⊆ current (by fullness).
    by_contra h_not_mem
    have h_in_step : r.head ∈ Closure.deltaStep t current := by
      simp only [Closure.deltaStep, List.mem_dedup, List.mem_append]
      right; simp only [List.mem_filterMap]
      exact ⟨r, hr, by simp [hdef, hbody, h_not_mem]⟩
    -- deltaStep when full: every element of deltaStep is in current
    have hfull : t.allLiterals.length ≤ current.length := by omega
    have h_fix := deltaStep_fixpoint_of_full t current hnodup hsub hfull
    exact nodup_subset_length_absurd hnodup
      (by simp only [Closure.deltaStep]; exact List.nodup_dedup _)
      (fun x hx => mem_deltaStep_of_mem t current x hx)
      (by simp only [beq_iff_eq] at h_fix; exact h_fix)
      h_in_step h_not_mem
  | succ n ih =>
    simp only [Closure.deltaClose.go] at hbody
    simp only [Closure.deltaClose.go]
    split
    · -- Fixpoint: deltaStep t current has same length as current
      rename_i hlen
      split at hbody
      · -- hbody is about current (fixpoint confirmed in both goal and hyp)
        by_contra h_not_mem
        have h_in_step : r.head ∈ Closure.deltaStep t current := by
          simp only [Closure.deltaStep, List.mem_dedup, List.mem_append]
          right; simp only [List.mem_filterMap]
          exact ⟨r, hr, by simp [hdef, hbody, h_not_mem]⟩
        exact nodup_subset_length_absurd hnodup
          (by simp only [Closure.deltaStep]; exact List.nodup_dedup _)
          (fun x hx => mem_deltaStep_of_mem t current x hx)
          (by simp only [beq_iff_eq] at hlen; exact hlen)
          h_in_step h_not_mem
      · rename_i hlen2; exact absurd hlen hlen2
    · -- Not fixpoint: recurse with deltaStep
      rename_i hlen
      split at hbody
      · rename_i hlen2; exact absurd hlen2 hlen
      · -- Need to provide: deltaStep sub allLiterals and fuel bound
        have h_step_nodup : (Closure.deltaStep t current).Nodup := by
          simp only [Closure.deltaStep]; exact List.nodup_dedup _
        have h_step_sub := deltaStep_sub_allLiterals t current hnodup hsub
        -- deltaStep strictly increases length when not fixpoint
        have h_strict : current.length < (Closure.deltaStep t current).length := by
          simp only [beq_iff_eq] at hlen
          have h_le : current.toFinset.card ≤ (Closure.deltaStep t current).toFinset.card :=
            Finset.card_le_card (fun x hx =>
              List.mem_toFinset.mpr (mem_deltaStep_of_mem t current x (List.mem_toFinset.mp hx)))
          rw [List.toFinset_card_of_nodup hnodup,
              List.toFinset_card_of_nodup h_step_nodup] at h_le
          omega
        have h_fuel' : t.allLiterals.length - (Closure.deltaStep t current).length ≤ n := by
          -- allLiterals.length - current.length ≤ n + 1
          -- current.length < deltaStep.length
          -- deltaStep ⊆ allLiterals, so deltaStep.length ≤ allLiterals.length
          have h_step_le : (Closure.deltaStep t current).length ≤ t.allLiterals.length := by
            have h_all_nodup : t.allLiterals.Nodup := List.nodup_dedup _
            have : (Closure.deltaStep t current).toFinset.card ≤ t.allLiterals.toFinset.card :=
              Finset.card_le_card (fun x hx =>
                List.mem_toFinset.mpr (h_step_sub x (List.mem_toFinset.mp hx)))
            rw [List.toFinset_card_of_nodup h_step_nodup,
                List.toFinset_card_of_nodup h_all_nodup] at this
            exact this
          omega
        exact ih _ h_step_nodup h_step_sub h_fuel' hbody

private theorem deltaClose_init_sub_allLiterals (t : Theory) :
    ∀ x ∈ (t.facts.map (·.head)).dedup, x ∈ t.allLiterals := by
  intro x hx
  have := List.dedup_subset _ hx
  simp only [List.mem_map, Theory.facts, List.mem_filter] at this
  obtain ⟨r, ⟨hrmem, _⟩, hrfl⟩ := this; subst hrfl
  exact List.subset_dedup _ (List.mem_append.mpr (Or.inl (List.mem_map.mpr ⟨r, hrmem, rfl⟩)))

/-- Completeness of +D: if there exists a definite rule with head l whose body
    is satisfied in deltaClose t, then l ∈ deltaClose t.

    Note: requires that the theory has at most 1000 distinct literals, since
    deltaClose uses fuel=1000. For larger theories, use a custom fuel value. -/
theorem reason_plusD_complete (t : Theory) (l : Literal)
    (hsmall : t.allLiterals.length ≤ 1000)
    (hderiv : ∃ r ∈ t.rules, r.isDefinite = true ∧ r.head = l
              ∧ r.bodySatisfied (Closure.deltaClose t) = true) :
    l ∈ Closure.deltaClose t := by
  obtain ⟨r, hr, hdef, hhead, hbody⟩ := hderiv
  rw [← hhead]
  simp only [Closure.deltaClose]
  apply deltaClose_go_fixpoint' t _ 1000 (List.nodup_dedup _)
    (deltaClose_init_sub_allLiterals t) _ r hr hdef hbody
  -- Fuel bound: allLiterals.length - init.length ≤ 1000
  -- Since init.length ≥ 0 and allLiterals.length ≤ 1000
  omega

/-- The three-phase decomposition preserves the subset chain:
    delta ⊆ partial ⊆ lambda.
    This is the fundamental invariant that makes the decomposition valid. -/
theorem three_phase_subset_chain (t : Theory) :
    let delta := Closure.deltaClose t
    let lambda := Closure.lambdaClose t delta
    let partial_ := Closure.partialClose t delta lambda
    (∀ l, l ∈ delta → l ∈ partial_)
    ∧ (∀ l, l ∈ partial_ → l ∈ lambda) := by
  constructor
  · exact delta_subset_partial t
  · exact partial_subset_lambda t

end Properties
