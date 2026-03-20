import SpindleLean.Closure.Partial
import SpindleLean.Properties.Soundness

/-!
# SpindleLean.Properties.Consistency

Tag consistency and level coherence for the DL(d) closure computation.

## Tag consistency

For any theory and literal `q`:
- `+D q` and `-D q` cannot both hold.
- `+d q` and `-d q` cannot both hold.

## Level coherence

The definite and defeasible levels are coherent:
- `+D q` implies `+d q` (when the theory is consistent at `~q`).
- `¬(+d q)` implies `¬(+D q) ∨ +D ~q`.
-/

namespace Consistency

open Delta Lambda Soundness

/-! ### Tag consistency: definite level (+D / -D) -/

/-- **+D / -D tag consistency**: if `q ∈ +D`, then `q ∉ -D`.

    Proof: `-D` is defined as `allLiterals t` filtered by `¬(+D.contains q)`.
    Since `q ∈ +D`, `contains` is `true`, so the filter excludes `q`. -/
theorem plusD_minusD_exclusive (t : Theory) (fuel : Nat) (q : Literal)
    (hpD : q ∈ computePlusD t fuel) :
    q ∉ computeMinusD t fuel := by
  intro h
  simp only [computeMinusD, List.mem_filter] at h
  simp only [Bool.not_eq_true'] at h
  exact (Bool.eq_false_iff.mp h.2) (List.contains_iff_mem.mpr hpD)

/-- **-D/+D tag consistency** (symmetric): if `q ∈ -D`, then `q ∉ +D`. -/
theorem minusD_plusD_exclusive (t : Theory) (fuel : Nat) (q : Literal)
    (hmD : q ∈ computeMinusD t fuel) :
    q ∉ computePlusD t fuel :=
  fun hpD => plusD_minusD_exclusive t fuel q hpD hmD

/-! ### Tag consistency: defeasible level (+d / -d)

The defeasible closure keeps `+d` and `-d` disjoint throughout iteration.
New elements are drawn from the "undecided" pool (not in either set), and
`newMinusd` explicitly excludes `newPlusd` members.
-/

/-- Helper: `lambdaStep` preserves disjointness of `+d` and `-d`.
    If the input sets are disjoint (via `contains`), the output sets are too. -/
private theorem lambdaStep_preserves_disjoint (t : Theory) (plusD plusd minusd : List Literal)
    (h_disj : ∀ l ∈ plusd, minusd.contains l = false)
    (_h_disj2 : ∀ l ∈ minusd, plusd.contains l = false) :
    (∀ l ∈ (lambdaStep t plusD plusd minusd).1,
      (lambdaStep t plusD plusd minusd).2.contains l = false) := by
  intro l hl
  simp only [lambdaStep] at hl ⊢
  -- Goal: (minusd ++ newMinusd.filter ...).contains l = false
  apply Bool.eq_false_iff.mpr
  intro hc
  have hm := List.contains_iff_mem.mp hc
  rcases List.mem_append.mp hl with h_old | h_new
  · -- l ∈ old plusd
    rcases List.mem_append.mp hm with h1 | h2
    · -- l ∈ minusd: contradicts h_disj
      exact absurd (List.contains_iff_mem.mpr h1) (Bool.eq_false_iff.mp (h_disj l h_old))
    · -- l in newMinusd.filter: l was in undecided which has !plusd.contains l
      have h_filt := List.mem_filter.mp h2
      have h_nm_inner := List.mem_filter.mp h_filt.1
      simp only [Bool.and_eq_true, Bool.not_eq_true'] at h_nm_inner
      have h_undec := List.mem_filter.mp h_nm_inner.1
      simp only [Bool.and_eq_true, Bool.not_eq_true'] at h_undec
      -- h_undec.2.1 : plusd.contains l = false, but l ∈ plusd
      exact absurd (List.contains_iff_mem.mpr h_old) (Bool.eq_false_iff.mp h_undec.2.1)
  · -- l ∈ newPlusd.filter
    have h_filt := List.mem_filter.mp h_new
    have h_np := h_filt.1  -- l ∈ newPlusd = undecided.filter canProve
    have h_np_filt := List.mem_filter.mp h_np
    have h_undec := h_np_filt.1  -- l ∈ undecided
    have h_undec_filt := List.mem_filter.mp h_undec
    simp only [Bool.and_eq_true, Bool.not_eq_true'] at h_undec_filt
    rcases List.mem_append.mp hm with h1 | h2
    · -- l ∈ minusd: contradicts undecided having !minusd.contains l
      exact absurd (List.contains_iff_mem.mpr h1) (Bool.eq_false_iff.mp h_undec_filt.2.2)
    · -- l in newMinusd.filter: newMinusd filters with !newPlusd.contains l
      have h_nm := List.mem_filter.mp h2
      have h_nm2 := List.mem_filter.mp h_nm.1
      simp only [Bool.and_eq_true, Bool.not_eq_true'] at h_nm2
      -- h_nm2.2.1 : newPlusd.contains l = false, but l ∈ newPlusd
      exact absurd (List.contains_iff_mem.mpr h_np) (Bool.eq_false_iff.mp h_nm2.2.1)

/-- Helper: `lambdaLoop` preserves disjointness throughout iteration. -/
private theorem lambdaLoop_preserves_disjoint (t : Theory) (plusD plusd minusd : List Literal)
    (fuel : Nat)
    (h_disj : ∀ l ∈ plusd, minusd.contains l = false)
    (h_disj2 : ∀ l ∈ minusd, plusd.contains l = false) :
    ∀ l ∈ (lambdaLoop t plusD plusd minusd fuel).1,
      (lambdaLoop t plusD plusd minusd fuel).2.contains l = false := by
  induction fuel generalizing plusd minusd with
  | zero =>
    simp only [lambdaLoop]
    exact h_disj
  | succ n ih =>
    simp only [lambdaLoop]
    split
    · exact h_disj
    · apply ih
      · exact lambdaStep_preserves_disjoint t plusD plusd minusd h_disj h_disj2
      · -- Need symmetric disjointness for the step output
        intro l hl
        simp only [lambdaStep] at hl ⊢
        apply Bool.eq_false_iff.mpr
        intro hc
        have hm := List.contains_iff_mem.mp hc
        rcases List.mem_append.mp hl with h_old | h_new
        · -- l ∈ old minusd
          rcases List.mem_append.mp hm with h1 | h2
          · -- l ∈ plusd: contradicts h_disj2
            exact absurd (List.contains_iff_mem.mpr h1) (Bool.eq_false_iff.mp (h_disj2 l h_old))
          · -- l in newPlusd.filter: l was in undecided which has !minusd.contains l
            have h_filt := List.mem_filter.mp h2
            have h_np := List.mem_filter.mp h_filt.1
            have h_undec := List.mem_filter.mp h_np.1
            simp only [Bool.and_eq_true, Bool.not_eq_true'] at h_undec
            -- h_undec.2.2 : minusd.contains l = false, but l ∈ minusd
            exact absurd (List.contains_iff_mem.mpr h_old) (Bool.eq_false_iff.mp h_undec.2.2)
        · -- l ∈ newMinusd.filter: l came from undecided (!plusd.contains l)
          have h_filt := List.mem_filter.mp h_new
          have h_nm_inner := List.mem_filter.mp h_filt.1
          simp only [Bool.and_eq_true, Bool.not_eq_true'] at h_nm_inner
          have h_undec_nm := List.mem_filter.mp h_nm_inner.1
          simp only [Bool.and_eq_true, Bool.not_eq_true'] at h_undec_nm
          rcases List.mem_append.mp hm with h1 | h2
          · -- l ∈ plusd: contradicts undecided having !plusd.contains l
            exact absurd (List.contains_iff_mem.mpr h1) (Bool.eq_false_iff.mp h_undec_nm.2.1)
          · -- l ∈ newPlusd.filter: l ∈ newPlusd, but newMinusd filters with !newPlusd.contains l
            have h_np_filt := List.mem_filter.mp h2
            have h_np := h_np_filt.1  -- l ∈ newPlusd
            -- h_nm_inner.2.1 : newPlusd.contains l = false, but l ∈ newPlusd
            exact absurd (List.contains_iff_mem.mpr h_np) (Bool.eq_false_iff.mp h_nm_inner.2.1)

/-- Seeds are disjoint: `seedPlusd` and `seedMinusd` have no common elements. -/
private theorem seeds_disjoint (t : Theory) (plusD : List Literal) :
    ∀ l ∈ seedPlusd plusD, (seedMinusd t plusD).contains l = false := by
  intro l hl
  simp only [seedPlusd, List.mem_filter] at hl
  obtain ⟨_, h_no_comp⟩ := hl
  simp only [Bool.not_eq_true'] at h_no_comp
  -- l ∈ seedPlusd: +D l and ¬(+D ~l)
  -- seedMinusd requires: +D ~l, so l can't be in seedMinusd
  apply Bool.eq_false_iff.mpr
  intro hc
  have hm := List.contains_iff_mem.mp hc
  simp only [seedMinusd, List.mem_filter] at hm
  obtain ⟨_, h_comp_def⟩ := hm
  simp only [Bool.and_eq_true, Bool.not_eq_true'] at h_comp_def
  exact absurd h_comp_def.1 (Bool.eq_false_iff.mp h_no_comp)

/-- Seeds symmetric disjointness. -/
private theorem seeds_disjoint_sym (t : Theory) (plusD : List Literal) :
    ∀ l ∈ seedMinusd t plusD, (seedPlusd plusD).contains l = false := by
  intro l hl
  simp only [seedMinusd, List.mem_filter] at hl
  obtain ⟨_, h_comp⟩ := hl
  simp only [Bool.and_eq_true, Bool.not_eq_true'] at h_comp
  apply Bool.eq_false_iff.mpr
  intro hc
  have hm := List.contains_iff_mem.mp hc
  simp only [seedPlusd, List.mem_filter] at hm
  obtain ⟨_, h_no_comp⟩ := hm
  simp only [Bool.not_eq_true'] at h_no_comp
  -- seedMinusd has +D ~l, seedPlusd has ¬(+D ~l): contradiction
  exact absurd h_comp.1 (Bool.eq_false_iff.mp h_no_comp)

/-- **+d / -d tag consistency**: if `q ∈ +d`, then `q ∉ -d`.

    Proof: the defeasible closure maintains disjointness of `+d` and `-d` as an
    invariant through `lambdaLoop`. Seeds are disjoint (one requires `+D ~q`,
    the other requires `¬(+D ~q)`), and each step draws from the undecided pool
    with `newMinusd` explicitly excluding `newPlusd`. -/
theorem plusd_minusd_exclusive (t : Theory) (fuel : Nat) (q : Literal)
    (hpd : q ∈ computePlusd t fuel) :
    q ∉ computeMinusd t fuel := by
  simp only [computePlusd, computeMinusd, computeDefeasible] at hpd ⊢
  intro hmd
  have h := lambdaLoop_preserves_disjoint t (computePlusD t fuel)
    (seedPlusd (computePlusD t fuel)) (seedMinusd t (computePlusD t fuel)) fuel
    (seeds_disjoint t (computePlusD t fuel))
    (seeds_disjoint_sym t (computePlusD t fuel))
    q hpd
  exact absurd (List.contains_iff_mem.mpr hmd) (Bool.eq_false_iff.mp h)

/-- **-d/+d tag consistency** (symmetric): if `q ∈ -d`, then `q ∉ +d`. -/
theorem minusd_plusd_exclusive (t : Theory) (fuel : Nat) (q : Literal)
    (hmd : q ∈ computeMinusd t fuel) :
    q ∉ computePlusd t fuel :=
  fun hpd => plusd_minusd_exclusive t fuel q hpd hmd

/-! ### Level coherence -/

/-- **Level coherence (+D → +d)**: if `q` is definitely provable and its
    complement is not, then `q` is defeasibly provable.

    The condition `¬(+D ~q)` is necessary: if the definite closure is
    inconsistent (both `+D q` and `+D ~q`), the defeasible phase does not
    inherit `q` into `+d` because condition 2 of `+d` requires `-D ~q`. -/
theorem plusD_implies_plusd (t : Theory) (fuel : Nat) (q : Literal)
    (hpD : q ∈ computePlusD t fuel)
    (h_comp : (computePlusD t fuel).contains q.complement = false) :
    q ∈ computePlusd t fuel :=
  Lambda.plusD_subsumes_plusd t fuel q hpD h_comp

/-- **Level coherence (¬(+d) → ¬(+D) ∨ +D ~q)**: if `q` is not defeasibly
    provable, then either `q` is not definitely provable, or the complement `~q`
    is definitely provable (blocking the +d derivation).

    Equivalently (by contrapositive of `plusD_implies_plusd`):
    `+D q ∧ ¬(+D ~q) → +d q`. -/
theorem not_plusd_implies_not_plusD_or_comp (t : Theory) (fuel : Nat) (q : Literal)
    (h : q ∉ computePlusd t fuel) :
    q ∉ computePlusD t fuel ∨ q.complement ∈ computePlusD t fuel := by
  rcases Classical.em (q ∈ computePlusD t fuel) with hpD | hnpD
  · rcases Classical.em (q.complement ∈ computePlusD t fuel) with hcomp | hncomp
    · exact Or.inr hcomp
    · exact absurd (plusD_implies_plusd t fuel q hpD
          (Bool.eq_false_iff.mpr (fun hc => hncomp (List.contains_iff_mem.mp hc)))) h
  · exact Or.inl hnpD

end Consistency
