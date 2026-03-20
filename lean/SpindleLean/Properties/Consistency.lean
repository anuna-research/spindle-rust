import SpindleLean.Properties.Completeness

/-!
# SpindleLean.Properties.Consistency

Tag consistency and level coherence for the definite and defeasible closures.

## Main results

### Tag consistency
- `definite_consistency`: `+D q` and `-D q` cannot both hold for any literal `q`.
- `defeasible_consistency`: `+d q` and `-d q` cannot both hold for any literal `q`.

### Level coherence
- `level_coherence_plusD`: `+D q` implies `+d q` or `+D ~q`.
- `level_coherence_minusd`: `-d q` implies `-D q` or `+D ~q`.

## Proof strategy

Tag consistency at the definite level follows directly from the definition of `-D`
as the complement of `+D` within the theory's literals.

Tag consistency at the defeasible level is established by maintaining a disjointness
invariant through `lambdaStep` and `lambdaLoop`: the `+d` and `-d` sets start disjoint
(seeds), and each step preserves disjointness because (1) new literals come from the
undecided set (disjoint from both `+d` and `-d`), (2) new `-d` literals are explicitly
filtered against new `+d` literals.

Level coherence follows from the seeding strategy: `+D q` seeds `+d q` unless `+D ~q`
also holds, and the contrapositive gives `-d q → -D q ∨ +D ~q`.
-/

namespace Consistency

open Delta Lambda

/-! ### Helper lemmas -/

private theorem contains_of_mem {l : Literal} {s : List Literal}
    (h : l ∈ s) : s.contains l = true :=
  List.elem_eq_true_of_mem h

/-! ### Definite tag consistency -/

/-- **Definite Consistency**: `+D q` and `-D q` cannot both hold.
    Follows directly from the definition of `-D` as `allLiterals \ +D`. -/
theorem definite_consistency (t : Theory) (fuel : Nat) (q : Literal) :
    q ∈ computePlusD t fuel → q ∉ computeMinusD t fuel := by
  intro hplus hminus
  simp only [computeMinusD, List.mem_filter, Bool.not_eq_true'] at hminus
  exact absurd (contains_of_mem hplus) (Bool.eq_false_iff.mp hminus.2)

/-- Equivalent formulation: `+D` and `-D` are mutually exclusive. -/
theorem definite_consistency' (t : Theory) (fuel : Nat) (q : Literal) :
    ¬(q ∈ computePlusD t fuel ∧ q ∈ computeMinusD t fuel) :=
  fun ⟨h1, h2⟩ => definite_consistency t fuel q h1 h2

/-! ### Defeasible tag consistency -/

/-- The initial `+d` and `-d` seeds are disjoint. -/
private theorem seeds_disjoint (t : Theory) (plusD : List Literal) (l : Literal) :
    l ∈ seedPlusd plusD → l ∉ seedMinusd t plusD := by
  intro hp hm
  simp only [seedPlusd, List.mem_filter, Bool.not_eq_true'] at hp
  simp only [seedMinusd, List.mem_filter, Bool.and_eq_true, Bool.not_eq_true'] at hm
  exact absurd hm.2.1 (Bool.eq_false_iff.mp hp.2)

/-- `lambdaStep` preserves disjointness of `+d` and `-d`.

    The proof uses a 4-case analysis on whether `l` comes from the old set or the
    new additions in each component:
    1. Old `+d` ∩ Old `-d`: contradicts the hypothesis.
    2. Old `+d` ∩ New `-d`: new `-d` ⊆ undecided ⊆ complement of `+d`.
    3. New `+d` ∩ Old `-d`: new `+d` ⊆ undecided ⊆ complement of `-d`.
    4. New `+d` ∩ New `-d`: new `-d` is filtered against new `+d`. -/
theorem lambdaStep_disjoint (t : Theory) (plusD plusd minusd : List Literal)
    (hdisj : ∀ l, l ∈ plusd → l ∉ minusd) :
    ∀ l, l ∈ (lambdaStep t plusD plusd minusd).1 →
         l ∉ (lambdaStep t plusD plusd minusd).2 := by
  intro l h1 h2
  simp only [lambdaStep] at h1 h2
  rcases List.mem_append.mp h1 with h1_old | h1_new <;>
    rcases List.mem_append.mp h2 with h2_old | h2_new
  · -- Case 1: l ∈ plusd ∧ l ∈ minusd
    exact hdisj l h1_old h2_old
  · -- Case 2: l ∈ plusd ∧ l ∈ newMinusd (filtered)
    -- newMinusd ⊆ undecided, which requires plusd.contains l = false
    have hf1 := (List.mem_filter.mp h2_new).1
    have hf2 := (List.mem_filter.mp hf1).1
    have hcond := (List.mem_filter.mp hf2).2
    simp only [Bool.and_eq_true, Bool.not_eq_true'] at hcond
    exact absurd (contains_of_mem h1_old) (Bool.eq_false_iff.mp hcond.1)
  · -- Case 3: l ∈ newPlusd (filtered) ∧ l ∈ minusd
    -- newPlusd ⊆ undecided, which requires minusd.contains l = false
    have hf1 := (List.mem_filter.mp h1_new).1
    have hf2 := (List.mem_filter.mp hf1).1
    have hcond := (List.mem_filter.mp hf2).2
    simp only [Bool.and_eq_true, Bool.not_eq_true'] at hcond
    exact absurd (contains_of_mem h2_old) (Bool.eq_false_iff.mp hcond.2)
  · -- Case 4: l ∈ newPlusd (filtered) ∧ l ∈ newMinusd (filtered)
    -- l ∈ newPlusd → newPlusd.contains l = true
    -- newMinusd filter requires !newPlusd.contains l → contradiction
    have h_in_np := (List.mem_filter.mp h1_new).1
    have h_np_contains := contains_of_mem h_in_np
    have hf_nm := (List.mem_filter.mp h2_new).1
    have hcond_nm := (List.mem_filter.mp hf_nm).2
    simp only [Bool.and_eq_true, Bool.not_eq_true'] at hcond_nm
    exact absurd h_np_contains (Bool.eq_false_iff.mp hcond_nm.1)

/-- `lambdaLoop` preserves disjointness of `+d` and `-d`, by induction on fuel. -/
theorem lambdaLoop_disjoint (t : Theory) (plusD plusd minusd : List Literal) (fuel : Nat)
    (hdisj : ∀ l, l ∈ plusd → l ∉ minusd) :
    ∀ l, l ∈ (lambdaLoop t plusD plusd minusd fuel).1 →
         l ∉ (lambdaLoop t plusD plusd minusd fuel).2 := by
  induction fuel generalizing plusd minusd with
  | zero => exact hdisj
  | succ n ih =>
    simp only [lambdaLoop]
    split
    · exact hdisj
    · exact ih _ _ (lambdaStep_disjoint t plusD plusd minusd hdisj)

/-- **Defeasible Consistency**: `+d q` and `-d q` cannot both hold. -/
theorem defeasible_consistency (t : Theory) (fuel : Nat) (q : Literal) :
    q ∈ computePlusd t fuel → q ∉ computeMinusd t fuel := by
  intro h1 h2
  simp only [computePlusd, computeMinusd, computeDefeasible] at h1 h2
  exact lambdaLoop_disjoint t _ _ _ fuel (seeds_disjoint t _) q h1 h2

/-- Equivalent formulation: `+d` and `-d` are mutually exclusive. -/
theorem defeasible_consistency' (t : Theory) (fuel : Nat) (q : Literal) :
    ¬(q ∈ computePlusd t fuel ∧ q ∈ computeMinusd t fuel) :=
  fun ⟨h1, h2⟩ => defeasible_consistency t fuel q h1 h2

/-! ### Level coherence -/

/-- **Level Coherence (+D → +d)**: if `+D q`, then either `+d q` or `+D ~q`.

    In a consistent theory (where `+D ~q` never holds when `+D q` does),
    this reduces to the standard `+D q → +d q`. -/
theorem level_coherence_plusD (t : Theory) (fuel : Nat) (q : Literal)
    (h : q ∈ computePlusD t fuel) :
    q ∈ computePlusd t fuel ∨ q.complement ∈ computePlusD t fuel := by
  cases hc : (computePlusD t fuel).contains q.complement
  · exact Or.inl (plusD_subsumes_plusd t fuel q h hc)
  · exact Or.inr (List.contains_iff_mem.mp hc)

/-- **Level Coherence (-d → -D)**: if `-d q`, then either `-D q` or `+D ~q`.

    Equivalently (contrapositive): `+D q ∧ -D ~q → +d q`. -/
theorem level_coherence_minusd (t : Theory) (fuel : Nat) (q : Literal)
    (h : q ∈ computeMinusd t fuel) :
    q ∉ computePlusD t fuel ∨ q.complement ∈ computePlusD t fuel := by
  by_cases hplusD : q ∈ computePlusD t fuel
  · rcases level_coherence_plusD t fuel q hplusD with hplusd | hcomp
    · exact absurd h (defeasible_consistency t fuel q hplusd)
    · exact Or.inr hcomp
  · exact Or.inl hplusD

end Consistency
