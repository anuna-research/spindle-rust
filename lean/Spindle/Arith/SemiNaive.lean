/-
  Spindle Semi-Naive Evaluation

  Formalizes semi-naive Datalog evaluation: starting from facts, iteratively
  derive new ground atoms by matching rule bodies against known facts, then
  instantiate rule heads. Defines the immediate consequence operator T_P
  and proves T_P is monotone.
-/
import Spindle.Arith.HerbrandBase

namespace Spindle.Arith

/-! ## BEq-based interpretations

An interpretation is a set of ground literals (facts).
We represent it as a `List Literal` and define subset using `BEq`-based
element checking (`List.elem`), avoiding the need for `LawfulBEq`. -/

/-- An interpretation: a finite set of ground atoms. -/
abbrev Interpretation := List Literal

/-- BEq-based membership: `l` is in `I` according to `BEq`. -/
def Interpretation.bmem (l : Literal) (I : Interpretation) : Bool :=
  I.elem l

/-- Subset relation on interpretations (BEq-based):
    every element found in `I` is also found in `J`. -/
def Interpretation.bsubset (I J : Interpretation) : Prop :=
  ∀ l, I.elem l = true → J.elem l = true

/-- Subset is reflexive. -/
theorem Interpretation.bsubset_refl (I : Interpretation) :
    Interpretation.bsubset I I :=
  fun _ h => h

/-- Subset is transitive. -/
theorem Interpretation.bsubset_trans {I J K : Interpretation}
    (hij : Interpretation.bsubset I J) (hjk : Interpretation.bsubset J K) :
    Interpretation.bsubset I K :=
  fun l h => hjk l (hij l h)

/-! ## Elem monotonicity for list append -/

/-- If `l` is found by `elem` in `I`, it is also found in `I ++ J`. -/
theorem elem_append_left (l : Literal) (I J : Interpretation)
    (h : I.elem l = true) : (I ++ J).elem l = true := by
  induction I with
  | nil => simp [List.elem] at h
  | cons x xs ih =>
    simp only [List.cons_append, List.elem] at h ⊢
    cases hbeq : (l == x)
    · simp_all
    · simp_all

/-- If `l` is found by `elem` in `J`, it is also found in `I ++ J`. -/
theorem elem_append_right (l : Literal) (I J : Interpretation)
    (h : J.elem l = true) : (I ++ J).elem l = true := by
  induction I with
  | nil => exact h
  | cons x xs ih =>
    simp only [List.cons_append, List.elem]
    cases hbeq : (l == x)
    · exact ih
    · simp_all

/-! ## Rule firing

A ground rule fires when all its body literals are present in the
current interpretation. -/

/-- A ground rule fires under interpretation `I` (BEq-based):
    every body literal is found in `I` by `elem`. -/
def Rule.firesb (r : Rule) (I : Interpretation) : Bool :=
  r.body.all (fun l => I.elem l)

/-- If `bsubset I J` and a rule fires under I, it fires under J. -/
theorem Rule.firesb_mono {I J : Interpretation}
    (hsub : Interpretation.bsubset I J)
    (r : Rule) (hfire : r.firesb I = true) : r.firesb J = true := by
  simp only [Rule.firesb, List.all_eq_true] at hfire ⊢
  intro l hl
  exact hsub l (hfire l hl)

/-! ## Immediate consequence operator T_P

T_P(I) collects the heads of all ground rules whose bodies are satisfied by I. -/

/-- The immediate consequence operator: given a ground program `P` and
    interpretation `I`, returns the set of heads of rules in `P` whose
    bodies are all in `I`. -/
def T_P (P : List Rule) (I : Interpretation) : Interpretation :=
  (P.filter (fun r => r.firesb I)).map Rule.head

/-- Equivalent characterization: `l ∈ T_P P I` iff there exists a rule
    in P with head `l` whose body is satisfied by `I`. -/
theorem mem_T_P_iff (P : List Rule) (I : Interpretation) (l : Literal) :
    l ∈ T_P P I ↔ ∃ r ∈ P, r.head = l ∧ r.firesb I = true := by
  simp only [T_P, List.mem_map, List.mem_filter]
  constructor
  · rintro ⟨r, ⟨hrP, hrFire⟩, rfl⟩
    exact ⟨r, hrP, rfl, hrFire⟩
  · rintro ⟨r, hrP, rfl, hrFire⟩
    exact ⟨r, ⟨hrP, hrFire⟩, rfl⟩

/-- If a rule fires in I and `bsubset I J`, the same rule fires in J,
    so the resulting filter on P for J is a superset. -/
private theorem filter_firesb_mono (P : List Rule) {I J : Interpretation}
    (hsub : Interpretation.bsubset I J) :
    ∀ r, r ∈ P.filter (fun r => r.firesb I) →
         r ∈ P.filter (fun r => r.firesb J) := by
  intro r hr
  simp only [List.mem_filter] at hr ⊢
  exact ⟨hr.1, Rule.firesb_mono hsub r hr.2⟩

/-! ## Monotonicity of T_P -/

/-- **Monotonicity of T_P**: if `bsubset I J` then every literal derived
    from `I` is also derived from `J`.

    This is the key property that ensures the semi-naive iteration
    converges to a fixpoint. Since T_P only checks membership of body
    literals in the interpretation, enlarging the interpretation can only
    cause more rules to fire, never fewer. -/
theorem T_P_mono (P : List Rule) {I J : Interpretation}
    (hsub : Interpretation.bsubset I J) :
    ∀ l, l ∈ T_P P I → l ∈ T_P P J := by
  intro l hl
  rw [mem_T_P_iff] at hl ⊢
  obtain ⟨r, hrP, hrHead, hrFire⟩ := hl
  exact ⟨r, hrP, hrHead, Rule.firesb_mono hsub r hrFire⟩

/-! ## Iterated consequence operator

Semi-naive evaluation computes the least fixpoint of T_P by iterating
from the initial facts. We define the iteration and prove basic
properties. -/

/-- Iterated T_P: apply the consequence operator `n` times starting from
    the initial set of facts, accumulating all derived atoms. -/
def T_P_iter (P : List Rule) (I₀ : Interpretation) : Nat → Interpretation
  | 0 => I₀
  | n + 1 => T_P_iter P I₀ n ++ T_P P (T_P_iter P I₀ n)

/-- Helper: bsubset is preserved by appending on the right. -/
theorem bsubset_append_left (I J : Interpretation) :
    Interpretation.bsubset I (I ++ J) :=
  fun l h => elem_append_left l I J h

/-- The base facts are always included in the iterated result. -/
theorem T_P_iter_base_bsubset (P : List Rule) (I₀ : Interpretation) (n : Nat) :
    Interpretation.bsubset I₀ (T_P_iter P I₀ n) := by
  induction n with
  | zero => exact Interpretation.bsubset_refl I₀
  | succ n ih =>
    exact Interpretation.bsubset_trans ih (bsubset_append_left _ _)

/-- Each iteration is a superset of the previous one. -/
theorem T_P_iter_mono (P : List Rule) (I₀ : Interpretation) (n : Nat) :
    Interpretation.bsubset (T_P_iter P I₀ n) (T_P_iter P I₀ (n + 1)) :=
  bsubset_append_left _ _

/-- The iteration chain is cumulative: m ≤ n implies iter m ⊆ iter n. -/
theorem T_P_iter_cumulative (P : List Rule) (I₀ : Interpretation) {m n : Nat}
    (h : m ≤ n) :
    Interpretation.bsubset (T_P_iter P I₀ m) (T_P_iter P I₀ n) := by
  induction h with
  | refl => exact Interpretation.bsubset_refl _
  | step _ ih => exact Interpretation.bsubset_trans ih (T_P_iter_mono P I₀ _)

/-! ## Facts from ground rules with empty bodies -/

/-- Extract the initial facts from a program: heads of rules with empty bodies. -/
def extractFacts (P : List Rule) : Interpretation :=
  (P.filter (fun r => r.body.isEmpty)).map Rule.head

/-- Every fact extracted is the head of some rule in P with empty body. -/
theorem mem_extractFacts (P : List Rule) (l : Literal) :
    l ∈ extractFacts P ↔ ∃ r ∈ P, r.body = [] ∧ r.head = l := by
  simp only [extractFacts, List.mem_map, List.mem_filter]
  constructor
  · rintro ⟨r, ⟨hrP, hrEmpty⟩, rfl⟩
    exact ⟨r, hrP, List.isEmpty_iff.mp hrEmpty, rfl⟩
  · rintro ⟨r, hrP, hrEmpty, rfl⟩
    exact ⟨r, ⟨hrP, List.isEmpty_iff.mpr hrEmpty⟩, rfl⟩

/-- Facts are always derived in the first T_P step from empty interpretation. -/
theorem extractFacts_subset_T_P (P : List Rule) :
    ∀ l, l ∈ extractFacts P → l ∈ T_P P [] := by
  intro l hl
  rw [mem_T_P_iff]
  rw [mem_extractFacts] at hl
  obtain ⟨r, hrP, hrBody, hrHead⟩ := hl
  refine ⟨r, hrP, hrHead, ?_⟩
  simp only [Rule.firesb, hrBody, List.all_nil]

/-! ## Semi-naive evaluation (bounded) -/

/-- Semi-naive evaluation with fuel: iterate T_P from the program's facts
    up to `fuel` steps. Returns the accumulated interpretation. -/
def semiNaiveEval (P : List Rule) (fuel : Nat) : Interpretation :=
  T_P_iter P (extractFacts P) fuel

end Spindle.Arith
