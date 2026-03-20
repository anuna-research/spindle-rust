import SpindleLean.Properties.Soundness

/-!
# SpindleLean.Properties.Completeness

Completeness of the definite closure computation with respect to the proof theory.

## Main result

`definite_completeness`: if a literal `q` satisfies the proof-theoretic condition
for `-D` — namely, `q` is not a fact in the theory AND every strict rule for `q`
has some body literal not in `+D` — then the algorithm marks `q` as `-D`
(i.e., `q ∈ computeMinusD`).

## Proof strategy

By contradiction: assume `q` is *not* marked `-D`, so `q ∈ +D`. By
`definite_soundness`, there exists a strict rule `r` for `q` whose body literals
are all in `+D`. If `r` is a fact, this contradicts the "not a fact" condition.
Otherwise, `r` having all body literals in `+D` contradicts the hypothesis that
every strict rule for `q` has a discarded body literal.
-/

namespace Completeness

open Delta Soundness

/-! ### Proof-theoretic condition for -D -/

/-- The proof-theoretic condition for `-D q`: `q` is not the head of any fact
    in the theory, and every strict rule for `q` has some body literal
    not in the `+D` set. -/
def minusD_condition (t : Theory) (q : Literal) (plusD : List Literal) : Prop :=
  (¬ ∃ r ∈ t.rules, r.isFact = true ∧ r.head = q) ∧
  (∀ r ∈ t.rules, r.ruleType = .strict → r.head = q → ∃ l ∈ r.body, l ∉ plusD)

/-! ### Core lemma: the -D condition implies not in +D -/

/-- If `q` satisfies the `-D` proof-theoretic condition with respect to
    `computePlusD t fuel`, then `q ∉ computePlusD t fuel`.

    Proof by contradiction using `definite_soundness`. -/
theorem not_in_plusD_of_minusD_condition (t : Theory) (fuel : Nat) (q : Literal)
    (hcond : minusD_condition t q (computePlusD t fuel)) :
    q ∉ computePlusD t fuel := by
  intro h_in
  obtain ⟨h_not_fact, h_all_discarded⟩ := hcond
  obtain ⟨r, hr_rules, hr_strict, hr_head, hr_body⟩ := definite_soundness t fuel q h_in
  -- Every strict rule for q must have a body literal not in +D
  obtain ⟨l, hl_body, hl_not_plusD⟩ := h_all_discarded r hr_rules hr_strict hr_head
  -- But soundness says all body literals of r are in +D
  exact hl_not_plusD (hr_body l hl_body)

/-! ### Main theorem -/

/-- **Definite Completeness**: if `q` satisfies the proof-theoretic condition
    for `-D` — not a fact, and every strict rule for `q` has a body literal
    not in `+D` — and `q` is mentioned in the theory, then `q ∈ computeMinusD`.

    Together with `definite_soundness`, this establishes that the algorithm's
    `+D`/`-D` classification exactly matches the proof-theoretic definition. -/
theorem definite_completeness (t : Theory) (fuel : Nat) (q : Literal)
    (hl : q ∈ allLiterals t)
    (hcond : minusD_condition t q (computePlusD t fuel)) :
    q ∈ computeMinusD t fuel := by
  simp only [computeMinusD]
  apply List.mem_filter.mpr
  constructor
  · exact hl
  · simp only [Bool.not_eq_true']
    exact Bool.eq_false_iff.mpr
      (fun h => not_in_plusD_of_minusD_condition t fuel q hcond (List.contains_iff_mem.mp h))

/-- **Definite Completeness (contrapositive form)**: if `q ∈ +D`, then the
    `-D` proof-theoretic condition does *not* hold — either `q` is a fact
    or some strict rule for `q` has all body literals in `+D`. -/
theorem plusD_implies_not_minusD_condition (t : Theory) (fuel : Nat) (q : Literal)
    (h : q ∈ computePlusD t fuel) :
    ¬ minusD_condition t q (computePlusD t fuel) :=
  fun hcond => not_in_plusD_of_minusD_condition t fuel q hcond h

end Completeness
