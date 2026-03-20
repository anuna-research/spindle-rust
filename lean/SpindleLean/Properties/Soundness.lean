import SpindleLean.Closure.Partial

/-!
# SpindleLean.Properties.Soundness

Soundness of the definite closure computation with respect to the proof theory.

## Main result

`definite_soundness`: if the definite closure marks `q` as `+D`, then either
`q` is a fact in the theory, or there exists a strict rule `r` with `r.head = q`
whose body literals are all in `+D`.

## Proof strategy

The proof proceeds by induction on the fuel parameter of `deltaLoop`. At each
iteration, newly added literals must come from applicable strict rules.
Monotonicity of the closure (each step only adds literals) ensures that body
literals present at an earlier step remain in the final output.
-/

namespace Soundness

open Delta

/-! ### Auxiliary lemmas for `List.contains` and membership -/

private theorem mem_of_contains {l : Literal} {s : List Literal}
    (h : s.contains l = true) : l ∈ s :=
  List.mem_of_elem_eq_true h

private theorem contains_of_mem {l : Literal} {s : List Literal}
    (h : l ∈ s) : s.contains l = true :=
  List.elem_eq_true_of_mem h

/-! ### BEq-to-Eq conversion for RuleType -/

private theorem ruleType_eq_of_beq {rt : RuleType}
    (h : (rt == RuleType.strict) = true) : rt = .strict := by
  cases rt
  · rfl
  all_goals (exfalso; revert h; decide)

/-! ### Helper lemmas -/

/-- `Rule.applicable` is monotone: if a rule is applicable in a smaller set,
    it is applicable in any superset. -/
theorem applicable_mono (r : Rule) (s₁ s₂ : List Literal)
    (hsub : ∀ l ∈ s₁, l ∈ s₂)
    (h : r.applicable s₁ = true) : r.applicable s₂ = true := by
  simp only [Rule.applicable] at *
  rw [List.all_eq_true] at *
  intro x hx
  exact contains_of_mem (hsub _ (mem_of_contains (h x hx)))

/-- If `r.applicable proved = true`, then every body literal is in `proved`. -/
theorem body_mem_of_applicable (r : Rule) (proved : List Literal)
    (h : r.applicable proved = true) :
    ∀ l ∈ r.body, l ∈ proved := by
  simp only [Rule.applicable] at h
  rw [List.all_eq_true] at h
  intro l hl
  exact mem_of_contains (h l hl)

/-! ### deltaStep characterization -/

/-- Every member of `deltaStep t proved` is either already in `proved` or
    is the head of an applicable strict rule in the theory. -/
theorem mem_deltaStep (t : Theory) (proved : List Literal) (q : Literal)
    (hq : q ∈ deltaStep t proved) :
    q ∈ proved ∨
    (∃ r, r ∈ t.rules ∧ r.ruleType = .strict ∧ r.head = q ∧
      r.applicable proved = true) := by
  simp only [deltaStep] at hq
  rcases List.mem_append.mp hq with h | h
  · exact Or.inl h
  · right
    have ⟨hm, _⟩ := List.mem_filter.mp h
    obtain ⟨r, hr, rfl⟩ := List.mem_map.mp hm
    simp only [Theory.strictRules] at hr
    have ⟨hr1, happ⟩ := List.mem_filter.mp hr
    have ⟨hr_rules, hr_strict_beq⟩ := List.mem_filter.mp hr1
    exact ⟨r, hr_rules, ruleType_eq_of_beq hr_strict_beq, rfl, happ⟩

/-! ### Main induction: deltaLoop soundness -/

/-- **deltaLoop soundness**: every literal in the output of `deltaLoop` was
    either in the initial `proved` set, or is the head of a strict rule in
    the theory whose body literals are all in the output.

    This is the core lemma, proved by induction on the fuel parameter. -/
theorem deltaLoop_sound (t : Theory) (proved : List Literal) (fuel : Nat)
    (q : Literal) (hq : q ∈ deltaLoop t proved fuel) :
    q ∈ proved ∨
    (∃ r, r ∈ t.rules ∧ r.ruleType = .strict ∧ r.head = q ∧
      ∀ l ∈ r.body, l ∈ deltaLoop t proved fuel) := by
  induction fuel generalizing proved with
  | zero =>
    exact Or.inl hq
  | succ n ih =>
    simp only [deltaLoop] at hq ⊢
    split at hq <;> rename_i heq
    · -- Fixed point reached
      simp only [show (deltaStep t proved == proved) = true from heq, ite_true]
      exact Or.inl hq
    · -- Recursive case
      simp only [show ¬(deltaStep t proved == proved) = true from heq]
      rcases ih (deltaStep t proved) hq with h_in_step | ⟨r, hr_rules, hr_strict, hr_head, hr_body⟩
      · rcases mem_deltaStep t proved q h_in_step with h_proved | ⟨r, hr_rules, hr_strict, hr_head, hr_app⟩
        · exact Or.inl h_proved
        · right
          exact ⟨r, hr_rules, hr_strict, hr_head, fun l hl => by
            have h_l_proved := body_mem_of_applicable r proved hr_app l hl
            exact deltaLoop_preserves t (deltaStep t proved) n l
              (deltaStep_extensive t proved l h_l_proved)⟩
      · right
        exact ⟨r, hr_rules, hr_strict, hr_head, hr_body⟩

/-! ### Fact seeds characterization -/

/-- If `q` is in the fact-head seeds, then there is a fact in the theory
    with head `q`. -/
theorem mem_fact_seeds (t : Theory) (q : Literal)
    (hq : q ∈ (t.facts.map Rule.head)) :
    ∃ r, r ∈ t.rules ∧ r.isFact = true ∧ r.head = q := by
  obtain ⟨r, hr, rfl⟩ := List.mem_map.mp hq
  simp only [Theory.facts] at hr
  have ⟨hr_rules, hr_fact⟩ := List.mem_filter.mp hr
  exact ⟨r, hr_rules, hr_fact, rfl⟩

/-- A fact (strict rule with empty body) is a strict rule. -/
theorem strict_of_isFact {r : Rule} (h : r.isFact = true) : r.ruleType = .strict := by
  rcases r with ⟨_, rt, body, _⟩
  simp only [Rule.isFact, Bool.and_eq_true] at h
  obtain ⟨h1, _⟩ := h
  cases rt
  · rfl
  all_goals (exfalso; revert h1; decide)

/-- A fact has an empty body. -/
theorem body_empty_of_isFact {r : Rule} (h : r.isFact = true) : r.body = [] := by
  rcases r with ⟨_, rt, body, _⟩
  simp only [Rule.isFact, Bool.and_eq_true] at h
  obtain ⟨_, h2⟩ := h
  cases body with
  | nil => rfl
  | cons _ _ => simp [List.isEmpty] at h2

/-! ### Main theorem -/

/-- **Definite Soundness**: if the definite closure marks `q` as `+D`, then
    there exists a strict rule `r` in the theory with `r.head = q` such that
    every body literal of `r` is also in `+D`.

    This covers both cases from the informal statement:
    - If `r` is a fact (strict rule with empty body), the body condition
      holds vacuously.
    - If `r` has a non-empty body, every antecedent literal is definitely
      provable.

    Proof by induction on the iteration step at which `q` was added to
    the closure. -/
theorem definite_soundness (t : Theory) (fuel : Nat) (q : Literal)
    (h : q ∈ computePlusD t fuel) :
    ∃ r ∈ t.rules, r.ruleType = .strict ∧ r.head = q ∧
      ∀ l ∈ r.body, l ∈ computePlusD t fuel := by
  simp only [computePlusD] at h ⊢
  rcases deltaLoop_sound t _ fuel q h with h_seed | ⟨r, hr_rules, hr_strict, hr_head, hr_body⟩
  · obtain ⟨r, hr_rules, hr_fact, hr_head⟩ := mem_fact_seeds t q h_seed
    exact ⟨r, hr_rules, strict_of_isFact hr_fact, hr_head, fun l hl => by
      rw [body_empty_of_isFact hr_fact] at hl
      contradiction⟩
  · exact ⟨r, hr_rules, hr_strict, hr_head, hr_body⟩

/-- **Definite Soundness (disjunctive form)**: if the definite closure marks
    `q` as `+D`, then either `q` is a fact or there exists a strict rule with
    a non-empty body for `q` whose antecedents are all in `+D`. -/
theorem definite_soundness' (t : Theory) (fuel : Nat) (q : Literal)
    (h : q ∈ computePlusD t fuel) :
    (∃ r ∈ t.rules, r.isFact = true ∧ r.head = q) ∨
    (∃ r ∈ t.rules, r.ruleType = .strict ∧ r.body ≠ [] ∧ r.head = q ∧
      ∀ l ∈ r.body, l ∈ computePlusD t fuel) := by
  obtain ⟨r, hr_rules, hr_strict, hr_head, hr_body⟩ := definite_soundness t fuel q h
  by_cases hb : r.body = []
  · left
    refine ⟨r, hr_rules, ?_, hr_head⟩
    simp only [Rule.isFact, Bool.and_eq_true]
    exact ⟨by rw [hr_strict]; rfl, by rw [hb]; rfl⟩
  · right
    exact ⟨r, hr_rules, hr_strict, hb, hr_head, hr_body⟩

end Soundness
