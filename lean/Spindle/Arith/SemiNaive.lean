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

/-! ## Grounding Termination

Semi-naive evaluation reaches a fixpoint in at most |Universe| steps.
T_P is monotone on the finite lattice ⟨℘(U), ⊆⟩, so the ascending chain
I₀ ⊆ I₁ ⊆ ... can gain at most one new atom per step, and there are only
|U| atoms available. This is the standard Knaster-Tarski argument
specialized to finite powersets. -/

section Termination

/-! ### BEq reflexivity

BEq on Literal is derived from structural BEq on Term. For all term
variants, BEq coincides with propositional equality (no opaque Float). -/

private theorem Term.beq_self (t : Term) : (t == t) = true := by
  cases t with
  | symbol s => exact beq_self_eq_true s
  | integer n => exact beq_self_eq_true n
  | decimal n s =>
    show (n == n && s == s) = true
    rw [beq_self_eq_true, beq_self_eq_true]; rfl
  | «variable» v => exact beq_self_eq_true v

private theorem List.term_beq_self (xs : List Term) : (xs == xs) = true := by
  induction xs with
  | nil => rfl
  | cons x xs ih =>
    show (x == x && xs == xs) = true
    rw [Term.beq_self, ih]; rfl

theorem Literal.beq_self (l : Literal) : (l == l) = true := by
  cases l with
  | mk name neg args =>
    show (name == name && (neg == neg && args == args)) = true
    rw [beq_self_eq_true, beq_self_eq_true, List.term_beq_self]; rfl

/-- Propositional list membership implies BEq-based `elem`. -/
theorem elem_of_mem {l : Literal} {xs : Interpretation}
    (h : l ∈ xs) : xs.elem l = true := by
  induction xs with
  | nil => contradiction
  | cons x xs ih =>
    simp only [List.elem]
    cases List.mem_cons.mp h with
    | inl heq => rw [heq, Literal.beq_self]
    | inr hmem =>
      cases hbeq : (l == x)
      · exact ih hmem
      · rfl

/-! ### Fixpoint definition -/

/-- An interpretation `I` is a fixpoint of `T_P P` when every atom
    derivable in one step is already present (BEq-based). -/
def Interpretation.isFixpoint (P : List Rule) (I : Interpretation) : Prop :=
  ∀ l, l ∈ T_P P I → I.elem l = true

/-! ### Coverage measure -/

/-- The coverage of `I` w.r.t. universe `U`: how many universe atoms
    are BEq-found in `I`. This is the monotone measure driving the
    termination argument. -/
def coverage (I : Interpretation) (U : List Literal) : Nat :=
  (U.filter (fun l => I.elem l)).length

/-- Coverage is bounded by |U|. -/
theorem coverage_le (I : Interpretation) (U : List Literal) :
    coverage I U ≤ U.length :=
  List.length_filter_le _ _

/-- Filter length is monotone w.r.t. predicate implication. -/
private theorem filter_length_mono {p q : Literal → Bool}
    (xs : List Literal) (h : ∀ x, p x = true → q x = true) :
    (xs.filter p).length ≤ (xs.filter q).length := by
  induction xs with
  | nil => simp [List.filter]
  | cons x xs ih =>
    simp only [List.filter]
    cases hp : p x <;> cases hq : q x
    · exact ih
    · simp only [List.length_cons]; exact Nat.le_succ_of_le ih
    · exact absurd (h x (by simp [hp])) (by simp [hq])
    · simp only [List.length_cons]; exact Nat.succ_le_succ ih

/-- Coverage is monotone: if `bsubset I J` then `coverage I U ≤ coverage J U`. -/
theorem coverage_mono {I J : Interpretation} (U : List Literal)
    (hsub : Interpretation.bsubset I J) :
    coverage I U ≤ coverage J U :=
  filter_length_mono U hsub

/-- If `p` implies `q`, there exists `x ∈ xs` with `¬p x` and `q x`,
    then the filter with `q` is strictly longer than with `p`. -/
private theorem filter_length_strict {p q : Literal → Bool}
    (xs : List Literal) (hmono : ∀ x, p x = true → q x = true)
    {w : Literal} (hw : w ∈ xs) (hpw : p w = false) (hqw : q w = true) :
    (xs.filter p).length < (xs.filter q).length := by
  induction xs with
  | nil => contradiction
  | cons x xs ih =>
    simp only [List.filter]
    cases List.mem_cons.mp hw with
    | inl heq =>
      subst heq
      rw [hpw, hqw]
      simp only [List.length_cons]
      exact Nat.lt_succ_of_le (filter_length_mono xs hmono)
    | inr hmem =>
      cases hp : p x <;> cases hq : q x
      · exact ih hmem
      · simp only [List.length_cons]
        exact Nat.lt_succ_of_lt (ih hmem)
      · exact absurd (hmono x (by simp [hp])) (by simp [hq])
      · simp only [List.length_cons]
        exact Nat.succ_lt_succ (ih hmem)

/-! ### Bounded monotone sequences stabilize -/

/-- A bounded, monotone-non-decreasing sequence of naturals
    must stabilize: there exists `n ≤ B` with `f n = f (n + 1)`.

    Proof: if `f` were strictly increasing at every step `n ≤ B`,
    then `f (B+1) ≥ f 0 + B + 1 > B`, contradicting the bound. -/
theorem bounded_mono_stabilizes {B : Nat} (f : Nat → Nat)
    (hmono : ∀ n, f n ≤ f (n + 1))
    (hbound : ∀ n, f n ≤ B) :
    ∃ n, n ≤ B ∧ f n = f (n + 1) := by
  apply Classical.byContradiction
  intro hall
  -- hall : ¬ ∃ n, n ≤ B ∧ f n = f (n + 1)
  -- So for all n ≤ B, f n ≠ f (n + 1), hence f n < f (n + 1)
  have hstrict : ∀ n, n ≤ B → f n + 1 ≤ f (n + 1) := by
    intro n hn
    have hne : f n ≠ f (n + 1) := fun heq => hall ⟨n, hn, heq⟩
    have hle := hmono n
    exact Nat.lt_of_le_of_ne hle hne
  -- By induction: f 0 + k ≤ f k for all k ≤ B + 1
  have hgrow : ∀ k, k ≤ B + 1 → f 0 + k ≤ f k := by
    intro k hk
    induction k with
    | zero => omega
    | succ k ih =>
      have hk' : k ≤ B := by omega
      have := hstrict k hk'
      have := ih (by omega)
      omega
  -- f(B+1) ≥ f(0) + B + 1 ≥ B + 1 > B, contradicting bound
  have := hgrow (B + 1) (Nat.le_refl _)
  have := hbound (B + 1)
  omega

/-! ### Coverage stabilization implies fixpoint -/

/-- Coverage of T_P_iter is monotone at each step. -/
theorem coverage_T_P_iter_mono (P : List Rule) (I₀ : Interpretation)
    (U : List Literal) (n : Nat) :
    coverage (T_P_iter P I₀ n) U ≤ coverage (T_P_iter P I₀ (n + 1)) U :=
  coverage_mono U (T_P_iter_mono P I₀ n)

/-- If T_P only produces atoms in universe `U`, and coverage does not
    increase from step `n` to step `n + 1`, then iteration `n` is a fixpoint.

    Key argument: if some `l ∈ T_P P I` had `I.elem l = false`, then `l ∈ U`
    (by boundedness) and `(I ++ T_P P I).elem l = true` (by appending),
    so the filter for `I ++ T_P P I` would be strictly longer — contradicting
    equal coverage. -/
theorem fixpoint_of_coverage_stable (P : List Rule) (I : Interpretation)
    (U : List Literal)
    (hBound : ∀ J, ∀ l, l ∈ T_P P J → l ∈ U)
    (hStable : coverage (I ++ T_P P I) U = coverage I U) :
    Interpretation.isFixpoint P I := by
  intro l hl
  -- Case split on I.elem l
  cases hcase : I.elem l with
  | true => rfl
  | false =>
    -- l ∈ T_P P I, so l ∈ U by boundedness
    exfalso
    have hlU : l ∈ U := hBound I l hl
    have hlApp : (I ++ T_P P I).elem l = true :=
      elem_append_right l I (T_P P I) (elem_of_mem hl)
    -- Coverage strictly increases — contradicts hStable
    have hlt : coverage I U < coverage (I ++ T_P P I) U :=
      filter_length_strict U
        (fun x hx => elem_append_left x I (T_P P I) hx)
        hlU hcase hlApp
    omega

/-! ### Main termination theorem -/

/-- **Grounding termination**: for any ground program `P` whose derived atoms
    lie within a finite universe `U`, semi-naive evaluation reaches a fixpoint
    in at most `|U|` steps.

    This is the standard Knaster-Tarski fixpoint argument specialized to finite
    powersets: T_P is monotone (proved in `T_P_mono`), so the ascending chain
    I₀ ⊆ I₁ ⊆ ... gains at most one new atom per step, and there are only |U|
    atoms available. -/
theorem semiNaive_terminates (P : List Rule) (I₀ : Interpretation)
    (U : List Literal)
    (hBound : ∀ I, ∀ l, l ∈ T_P P I → l ∈ U) :
    ∃ n, n ≤ U.length ∧ Interpretation.isFixpoint P (T_P_iter P I₀ n) := by
  obtain ⟨n, hn, hstable⟩ := bounded_mono_stabilizes
    (fun n => coverage (T_P_iter P I₀ n) U)
    (coverage_T_P_iter_mono P I₀ U)
    (fun n => coverage_le _ U)
  refine ⟨n, hn, ?_⟩
  apply fixpoint_of_coverage_stable P (T_P_iter P I₀ n) U hBound
  simp only [T_P_iter] at hstable
  exact hstable.symm

/-- **Corollary**: semi-naive evaluation of a program from its own facts
    terminates within |theoryHerbrandBase| steps, provided all derived atoms
    are within the theory's Herbrand base. -/
theorem semiNaive_terminates_theory (P : List Rule)
    (hBound : ∀ I, ∀ l, l ∈ T_P P I → l ∈ theoryHerbrandBase P) :
    ∃ n, n ≤ (theoryHerbrandBase P).length ∧
      Interpretation.isFixpoint P (T_P_iter P (extractFacts P) n) :=
  semiNaive_terminates P (extractFacts P) (theoryHerbrandBase P) hBound

end Termination

end Spindle.Arith
