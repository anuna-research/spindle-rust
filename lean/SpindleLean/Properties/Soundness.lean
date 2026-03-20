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

/-! ### Definite Completeness (-D) -/

/-- **Definite Completeness (contrapositive form)**: if `q` is not in `+D`,
    then `q` is in `-D` (provided `q` is mentioned in the theory).

    This is immediate from the definition of `computeMinusD`. -/
theorem not_plusD_then_minusD (t : Theory) (fuel : Nat) (q : Literal)
    (h_in_theory : q ∈ Delta.allLiterals t)
    (h_not_plusD : q ∉ computePlusD t fuel) :
    q ∈ computeMinusD t fuel := by
  simp only [computeMinusD]
  exact List.mem_filter.mpr ⟨h_in_theory, by
    simp only [Bool.not_eq_true']
    exact Bool.eq_false_iff.mpr (fun h => h_not_plusD (List.contains_iff_mem.mp h))⟩

/-- **Definite Completeness**: if `q` is mentioned in the theory, and every
    strict rule with head `q` has at least one body literal not in `+D`, then
    `q ∈ computeMinusD t fuel` (i.e., the algorithm marks `q` as `-D`).

    Covers the `-D` condition from SPEC-015 REQ-005: `q` gets `-D` when there
    is no applicable strict derivation for `q`.

    Proof by contradiction using `definite_soundness`: if `q` were in `+D`,
    soundness gives a strict rule whose body is entirely in `+D`, contradicting
    the hypothesis that every such rule has an unsatisfied antecedent. -/
theorem definite_completeness (t : Theory) (fuel : Nat) (q : Literal)
    (h_in_theory : q ∈ Delta.allLiterals t)
    (h_no_strict_derivation : ∀ r ∈ t.rules, r.ruleType = .strict → r.head = q →
      ∃ l ∈ r.body, l ∉ computePlusD t fuel) :
    q ∈ computeMinusD t fuel := by
  apply not_plusD_then_minusD t fuel q h_in_theory
  intro h_in_plusD
  obtain ⟨r, hr_rules, hr_strict, hr_head, hr_body⟩ := definite_soundness t fuel q h_in_plusD
  obtain ⟨l, hl_body, hl_not_plusD⟩ := h_no_strict_derivation r hr_rules hr_strict hr_head
  exact hl_not_plusD (hr_body l hl_body)

/-- **Definite Completeness (iff form)**: `q ∈ computeMinusD t fuel` if and only
    if `q` is mentioned in the theory and `q ∉ computePlusD t fuel`.

    This captures the exact semantics of `-D`: a literal is definitely not
    provable precisely when it appears in the theory but is not in `+D`. -/
theorem minusD_iff (t : Theory) (fuel : Nat) (q : Literal) :
    q ∈ computeMinusD t fuel ↔
    q ∈ Delta.allLiterals t ∧ q ∉ computePlusD t fuel := by
  constructor
  · intro h
    simp only [computeMinusD, List.mem_filter] at h
    obtain ⟨h_in, h_not_contains⟩ := h
    exact ⟨h_in, fun h_in_plusD => by
      simp at h_not_contains
      exact h_not_contains h_in_plusD⟩
  · rintro ⟨h_in, h_not⟩
    exact not_plusD_then_minusD t fuel q h_in h_not

/-! ## Defeasible Soundness (+d)

If the defeasible closure marks `q` as `+d`, then either `+D q` holds (subsumption
from the definite phase), or the three defeasible proof conditions are satisfied:
1. ∃r ∈ Rsd[q] applicable in the final `+d` set
2. `-D ~q` (complement not definitely provable)
3. Every attacker for `~q` is countered (discarded or defeated by a superior rule)

### Proof strategy

Proof by induction on the `lambdaLoop` fuel parameter. At each iteration,
newly added literals satisfy `canProve` with the intermediate sets.
Monotonicity of `canProve` (since `applicable` and `discarded` are monotone
and the sets only grow) lifts this to the final sets.
-/

open Lambda in

/-! ### Monotonicity lemmas -/

/-- `Rule.discarded` is monotone: if a rule is discarded in a smaller set,
    it is discarded in any superset. -/
theorem discarded_mono (r : Rule) (s₁ s₂ : List Literal)
    (hsub : ∀ l ∈ s₁, l ∈ s₂)
    (h : r.discarded s₁ = true) : r.discarded s₂ = true := by
  simp only [Rule.discarded] at *
  rw [List.any_eq_true] at *
  obtain ⟨x, hx_body, hx_in⟩ := h
  exact ⟨x, hx_body, contains_of_mem (hsub _ (mem_of_contains hx_in))⟩

/-- `Lambda.attackerCountered` is monotone in both `plusd` (↑) and `minusd` (↑). -/
theorem attackerCountered_mono (t : Theory) (plusd₁ plusd₂ minusd₁ minusd₂ : List Literal)
    (s : Rule)
    (hp : ∀ l ∈ plusd₁, l ∈ plusd₂)
    (hm : ∀ l ∈ minusd₁, l ∈ minusd₂)
    (h : Lambda.attackerCountered t plusd₁ minusd₁ s = true) :
    Lambda.attackerCountered t plusd₂ minusd₂ s = true := by
  simp only [Lambda.attackerCountered] at *
  rw [Bool.or_eq_true] at h ⊢
  rcases h with hdisc | hany
  · exact Or.inl (discarded_mono s minusd₁ minusd₂ hm hdisc)
  · apply Or.inr
    rw [List.any_eq_true] at hany ⊢
    obtain ⟨supporter, hs_mem, hs_prop⟩ := hany
    exact ⟨supporter, hs_mem, by
      simp only [Bool.and_eq_true] at hs_prop ⊢
      exact ⟨applicable_mono supporter plusd₁ plusd₂ hp hs_prop.1, hs_prop.2⟩⟩

/-- `Lambda.canProve` is monotone in both `plusd` (↑) and `minusd` (↑).
    The `plusD` parameter (definite closure) is fixed. -/
theorem canProve_mono (t : Theory) (plusD : List Literal)
    (plusd₁ plusd₂ minusd₁ minusd₂ : List Literal)
    (q : Literal)
    (hp : ∀ l ∈ plusd₁, l ∈ plusd₂)
    (hm : ∀ l ∈ minusd₁, l ∈ minusd₂)
    (h : Lambda.canProve t plusD plusd₁ minusd₁ q = true) :
    Lambda.canProve t plusD plusd₂ minusd₂ q = true := by
  simp only [Lambda.canProve] at *
  simp only [Bool.and_eq_true] at *
  obtain ⟨⟨h_app, h_comp⟩, h_countered⟩ := h
  refine ⟨⟨?_, h_comp⟩, ?_⟩
  · -- hasApplicable: ∃r ∈ Rsd[q] applicable in plusd₁ → applicable in plusd₂
    rw [List.any_eq_true] at h_app ⊢
    obtain ⟨r, hr_mem, hr_app⟩ := h_app
    exact ⟨r, hr_mem, applicable_mono r plusd₁ plusd₂ hp hr_app⟩
  · -- allCountered: each attacker countered with (plusd₁, minusd₁) → (plusd₂, minusd₂)
    rw [List.all_eq_true] at h_countered ⊢
    intro s hs
    exact attackerCountered_mono t plusd₁ plusd₂ minusd₁ minusd₂ s hp hm (h_countered s hs)

/-! ### lambdaStep characterization -/

/-- If `q` is newly added to `+d` by `lambdaStep`, then `canProve` was true
    with the intermediate sets. -/
theorem mem_lambdaStep_plusd_new (t : Theory) (plusD plusd minusd : List Literal)
    (q : Literal) (hq : q ∈ (Lambda.lambdaStep t plusD plusd minusd).1)
    (hn : q ∉ plusd) :
    Lambda.canProve t plusD plusd minusd q = true := by
  simp only [Lambda.lambdaStep] at hq
  rcases List.mem_append.mp hq with h_old | h_new
  · exact absurd h_old hn
  · have h1 := List.mem_filter.mp h_new
    have h2 := List.mem_filter.mp h1.1
    exact h2.2

/-! ### lambdaLoop soundness -/

/-- Core induction: if `q ∈ lambdaLoop t plusD plusd minusd fuel` (i.e., `q` is
    in the final `+d` set), then either `q` was in the initial `plusd`, or
    `canProve` holds with the final `+d` and `-d` sets. -/
theorem lambdaLoop_sound (t : Theory) (plusD plusd minusd : List Literal)
    (fuel : Nat) (q : Literal)
    (hq : q ∈ (Lambda.lambdaLoop t plusD plusd minusd fuel).1) :
    q ∈ plusd ∨
    Lambda.canProve t plusD
      (Lambda.lambdaLoop t plusD plusd minusd fuel).1
      (Lambda.lambdaLoop t plusD plusd minusd fuel).2
      q = true := by
  induction fuel generalizing plusd minusd with
  | zero =>
    simp only [Lambda.lambdaLoop] at hq
    exact Or.inl hq
  | succ n ih =>
    simp only [Lambda.lambdaLoop] at hq ⊢
    split at hq <;> rename_i heq
    · -- Fixed point reached
      exact Or.inl hq
    · -- Recursive case
      -- Reduce the if-expression in the goal using the negated fixed-point condition
      have heq' : ((Lambda.lambdaStep t plusD plusd minusd).fst == plusd &&
                   (Lambda.lambdaStep t plusD plusd minusd).snd == minusd) = false :=
        (Bool.not_eq_true _).mp heq
      simp only [heq', Bool.false_eq_true, ite_false]
      rcases ih (Lambda.lambdaStep t plusD plusd minusd).1
               (Lambda.lambdaStep t plusD plusd minusd).2
               hq with h_in_step | h_canProve
      · -- q was in the step output's plusd
        by_cases h_old : q ∈ plusd
        · exact Or.inl h_old
        · -- q was newly added by this step
          have h_cp := mem_lambdaStep_plusd_new t plusD plusd minusd q h_in_step h_old
          right
          exact canProve_mono t plusD
            plusd
            (Lambda.lambdaLoop t plusD
              (Lambda.lambdaStep t plusD plusd minusd).1
              (Lambda.lambdaStep t plusD plusd minusd).2 n).1
            minusd
            (Lambda.lambdaLoop t plusD
              (Lambda.lambdaStep t plusD plusd minusd).1
              (Lambda.lambdaStep t plusD plusd minusd).2 n).2
            q
            (fun l hl => Lambda.lambdaLoop_preserves_plusd t plusD
              (Lambda.lambdaStep t plusD plusd minusd).1
              (Lambda.lambdaStep t plusD plusd minusd).2 n l
              (Lambda.lambdaStep_preserves_plusd t plusD plusd minusd l hl))
            (fun l hl => Lambda.lambdaLoop_preserves_minusd t plusD
              (Lambda.lambdaStep t plusD plusd minusd).1
              (Lambda.lambdaStep t plusD plusd minusd).2 n l
              (Lambda.lambdaStep_preserves_minusd t plusD plusd minusd l hl))
            h_cp
      · -- canProve already holds at final sets
        exact Or.inr h_canProve

/-! ### Seed characterization -/

/-- If `q` is in `seedPlusd plusD`, then `q ∈ plusD`. -/
theorem mem_seedPlusd (plusD : List Literal) (q : Literal)
    (h : q ∈ Lambda.seedPlusd plusD) : q ∈ plusD := by
  simp only [Lambda.seedPlusd] at h
  exact (List.mem_filter.mp h).1

/-! ### Main theorem -/

/-- **Defeasible Soundness**: if the defeasible closure marks `q` as `+d`, then
    either `q` is definitely provable (`+D`), or `canProve` holds with the final
    `+d` and `-d` sets — meaning:
    1. Some rule in `Rsd[q]` is applicable (body literals all in `+d`)
    2. The complement `~q` is not in `+D`
    3. Every attacker for `~q` is countered (discarded or defeated)

    This covers the `+d` condition from SPEC-015 REQ-006. -/
theorem defeasible_soundness (t : Theory) (fuel : Nat) (q : Literal)
    (h : q ∈ Lambda.computePlusd t fuel) :
    (q ∈ Delta.computePlusD t fuel) ∨
    (Lambda.canProve t (Delta.computePlusD t fuel)
      (Lambda.computePlusd t fuel) (Lambda.computeMinusd t fuel) q = true) := by
  simp only [Lambda.computePlusd, Lambda.computeMinusd, Lambda.computeDefeasible] at h ⊢
  rcases lambdaLoop_sound t (Delta.computePlusD t fuel)
    (Lambda.seedPlusd (Delta.computePlusD t fuel))
    (Lambda.seedMinusd t (Delta.computePlusD t fuel))
    fuel q h with h_seed | h_canProve
  · exact Or.inl (mem_seedPlusd (Delta.computePlusD t fuel) q h_seed)
  · exact Or.inr h_canProve

/-- **Defeasible Soundness (expanded form)**: unpacks `canProve` into its three
    explicit conditions for more direct use in downstream proofs. -/
theorem defeasible_soundness' (t : Theory) (fuel : Nat) (q : Literal)
    (h : q ∈ Lambda.computePlusd t fuel) :
    (q ∈ Delta.computePlusD t fuel) ∨
    ((t.Rsd q).any (fun r => r.applicable (Lambda.computePlusd t fuel)) = true ∧
     (Delta.computePlusD t fuel).contains q.complement = false ∧
     (t.R q.complement).all (fun s =>
       Lambda.attackerCountered t (Lambda.computePlusd t fuel)
         (Lambda.computeMinusd t fuel) s) = true) := by
  rcases defeasible_soundness t fuel q h with hD | hcp
  · exact Or.inl hD
  · right
    simp only [Lambda.canProve, Bool.and_eq_true] at hcp
    obtain ⟨⟨h1, h2⟩, h3⟩ := hcp
    exact ⟨h1, by simpa using h2, h3⟩

/-! ## Defeasible Completeness (-d)

If `q` does not satisfy the `+d` proof conditions, then `q` is not in `+d`.
Combined with coverage (every literal gets either `+d` or `-d`), this gives:
if the `-d` proof-theoretic conditions hold, `q` is marked `-d`.
-/

open Lambda in

/-- **Contrapositive of defeasible soundness**: if `q` is not in `+D` and
    `canProve` fails with the final sets, then `q ∉ +d`. -/
theorem not_plusd_of_no_derivation (t : Theory) (fuel : Nat) (q : Literal)
    (h_not_D : q ∉ Delta.computePlusD t fuel)
    (h_no_cp : Lambda.canProve t (Delta.computePlusD t fuel)
      (Lambda.computePlusd t fuel) (Lambda.computeMinusd t fuel) q ≠ true) :
    q ∉ Lambda.computePlusd t fuel := by
  intro h
  rcases defeasible_soundness t fuel q h with hD | hcp
  · exact h_not_D hD
  · exact h_no_cp hcp

/-- **Defeasible Completeness**: if `q` is mentioned in the theory and `q ∉ +d`,
    then `q` appears in the `-d` output of `defeasibleClosure`.

    This follows because `defeasibleClosure` assigns `-d` to every literal in
    the theory that is not in `+d` (either through `lambdaLoop`'s `-d` set or
    by collecting remaining undecided literals).

    Covers the `-d` condition from SPEC-015 REQ-007: any literal not satisfying
    the `+d` conditions is guaranteed to receive `-d`. -/
private theorem filter_minusd_map_literal (neg_lits : List Literal) :
    ((neg_lits.map (fun l => ({ conclusionType := .minusd, literal := l } : Conclusion))).filter
      (fun c => c.conclusionType == .minusd)).map Conclusion.literal = neg_lits := by
  induction neg_lits with
  | nil => simp
  | cons h t ih =>
    simp only [List.map, List.filter]
    simp only [show (ConclusionType.minusd == ConclusionType.minusd) = true from rfl]
    simp only [List.map, ih]

private theorem filter_plusd_minusd (pos_lits : List Literal) :
    (pos_lits.map (fun l => ({ conclusionType := .plusd, literal := l } : Conclusion))).filter
      (fun c => c.conclusionType == .minusd) = [] := by
  induction pos_lits with
  | nil => simp
  | cons h t ih =>
    simp only [List.map, List.filter]
    simp only [show (ConclusionType.plusd == ConclusionType.minusd) = false from rfl, ih]

theorem defeasible_completeness (t : Theory) (fuel : Nat) (q : Literal)
    (h_in : q ∈ Delta.allLiterals t)
    (h_not_plusd : q ∉ Lambda.computePlusd t fuel) :
    q ∈ (Lambda.defeasibleClosure t fuel |>.filter
      fun c => c.conclusionType == .minusd).map Conclusion.literal := by
  simp only [Lambda.defeasibleClosure]
  rw [List.filter_append, List.map_append]
  rw [filter_plusd_minusd, filter_minusd_map_literal]
  -- Goal: q ∈ map .literal [] ++ (minusd ++ remaining)
  simp only [List.map_nil, List.nil_append, List.mem_append]
  have h_not_plusd' : q ∉ (Lambda.computeDefeasible t fuel).1 := h_not_plusd
  by_cases hm : q ∈ (Lambda.computeDefeasible t fuel).2
  · exact Or.inl hm
  · apply Or.inr
    simp only [List.mem_filter, Bool.and_eq_true, Bool.not_eq_true']
    exact ⟨h_in,
           Bool.eq_false_iff.mpr (fun h => h_not_plusd' (List.contains_iff_mem.mp h)),
           Bool.eq_false_iff.mpr (fun h => hm (List.contains_iff_mem.mp h))⟩

end Soundness
