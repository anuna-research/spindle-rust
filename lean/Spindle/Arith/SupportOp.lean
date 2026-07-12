/-
  Spindle Support Reasoning Operator

  The CONCRETE monotone reasoning operator that instantiates the abstract
  `ReasoningOp` contract consumed by the what-if / abduce / why-not
  specifications: the support closure computed by semi-naive forward
  chaining (`T_P_iter`, SemiNaive.lean), which treats every rule as a
  strict implication and ignores defeat.

  Why this operator, and not the engine's defeasible conclusions:
  defeasible `+d` is NOT monotone — adding rules (e.g. the hypothetical
  fact `~p` to a theory deriving `+d p` from `a => p`, `a`) retracts
  conclusions. The machine-checked witness is
  `Properties.defeasible_not_monotone`
  (SpindleLean/Properties/NonMonotonicity.lean). The support closure is
  the monotone over-approximation of defeasible support (what the
  engine's grounding / reachability phase computes), and it is the
  fragment for which the monotone query theorems (`whatIf_mono`,
  `abduce_solution_superset`, `abduce_mono`, `abduce_union_valid`, ...)
  actually hold.

  Main results:
  - `supportOp : ReasoningOp` — the concrete instance; its `mono` field is
    discharged from `T_P` monotonicity in BOTH the program and the
    interpretation (`T_P_iter_program_mono`), so the abstract contract is
    witnessed by an executable reasoner rather than assumed.
  - `supportOp_derives_facts` — fact rules derive their own heads: the
    precondition consumed by `abduce_self_valid`.
  - `supportOp_abduce_self` — `abduce_self_valid` instantiated at the
    concrete operator.
-/
import Spindle.Arith.SemiNaive
import Spindle.Arith.Abduce

namespace Spindle.Arith

/-! ## BEq-based elem plumbing -/

/-- `elem` yields a BEq-equal member of the list. -/
theorem exists_beq_of_elem {l : Literal} {I : Interpretation}
    (h : I.elem l = true) : ∃ x ∈ I, (l == x) = true := by
  induction I with
  | nil => simp [List.elem] at h
  | cons y ys ih =>
    simp only [List.elem] at h
    cases hly : (l == y) with
    | true => exact ⟨y, List.mem_cons_self, hly⟩
    | false =>
      rw [hly] at h
      obtain ⟨x, hx, hbeq⟩ := ih h
      exact ⟨x, List.mem_cons_of_mem _ hx, hbeq⟩

/-- A BEq-equal member witnesses `elem`. -/
theorem elem_of_beq_of_mem {l x : Literal} {I : Interpretation}
    (hbeq : (l == x) = true) (hmem : x ∈ I) : I.elem l = true := by
  induction I with
  | nil => cases hmem
  | cons y ys ih =>
    simp only [List.elem]
    cases hly : (l == y) with
    | true => rfl
    | false =>
      cases List.mem_cons.mp hmem with
      | inl heq => subst heq; rw [hbeq] at hly; cases hly
      | inr h => exact ih h

/-- `elem` over an append splits. -/
theorem elem_append_split {l : Literal} {I J : Interpretation}
    (h : (I ++ J).elem l = true) :
    I.elem l = true ∨ J.elem l = true := by
  obtain ⟨x, hx, hbeq⟩ := exists_beq_of_elem h
  cases List.mem_append.mp hx with
  | inl hI => exact Or.inl (elem_of_beq_of_mem hbeq hI)
  | inr hJ => exact Or.inr (elem_of_beq_of_mem hbeq hJ)

/-! ## T_P and extractFacts are monotone in the PROGRAM -/

/-- `T_P` is monotone in both the program and the interpretation. -/
theorem T_P_full_mono {P₁ P₂ : List Rule} (hsub : ∀ r ∈ P₁, r ∈ P₂)
    {I J : Interpretation} (hIJ : Interpretation.bsubset I J) :
    ∀ l, (T_P P₁ I).elem l = true → (T_P P₂ J).elem l = true := by
  intro l hl
  obtain ⟨x, hx, hbeq⟩ := exists_beq_of_elem hl
  rw [mem_T_P_iff] at hx
  obtain ⟨r, hrP, hrHead, hrFire⟩ := hx
  exact elem_of_beq_of_mem hbeq
    ((mem_T_P_iff P₂ J x).mpr ⟨r, hsub r hrP, hrHead, Rule.firesb_mono hIJ r hrFire⟩)

/-- `extractFacts` is monotone in the program (elem-based). -/
theorem extractFacts_program_mono {P₁ P₂ : List Rule}
    (hsub : ∀ r ∈ P₁, r ∈ P₂) :
    Interpretation.bsubset (extractFacts P₁) (extractFacts P₂) := by
  intro l hl
  obtain ⟨x, hx, hbeq⟩ := exists_beq_of_elem hl
  rw [mem_extractFacts] at hx
  obtain ⟨r, hrP, hrBody, hrHead⟩ := hx
  exact elem_of_beq_of_mem hbeq
    ((mem_extractFacts P₂ x).mpr ⟨r, hsub r hrP, hrBody, hrHead⟩)

/-- The iterated consequence operator is monotone in the program and the
    seed, pointwise at every iteration count. -/
theorem T_P_iter_program_mono {P₁ P₂ : List Rule} (hsub : ∀ r ∈ P₁, r ∈ P₂)
    {I₀ J₀ : Interpretation} (h0 : Interpretation.bsubset I₀ J₀) (n : Nat) :
    Interpretation.bsubset (T_P_iter P₁ I₀ n) (T_P_iter P₂ J₀ n) := by
  induction n with
  | zero => exact h0
  | succ n ih =>
    intro l hl
    simp only [T_P_iter] at hl ⊢
    cases elem_append_split hl with
    | inl h => exact elem_append_left l _ _ (ih l h)
    | inr h => exact elem_append_right l _ _ (T_P_full_mono hsub ih l h)

/-! ## The concrete reasoning operator -/

/-- Support derivability: `l` is reached by some finite number of
    semi-naive iterations from the theory's facts, treating every rule as
    strict. This is the executable support closure (`semiNaiveEval`)
    quantified over fuel, i.e. its least fixed point. -/
def supportDerivable (T : Theory) (l : Literal) : Prop :=
  ∃ n, (T_P_iter T (extractFacts T) n).elem l = true

/-- **The concrete monotone reasoning operator.** Its monotonicity is a
    theorem about the semi-naive evaluator, not an assumption: enlarging
    the program enlarges both the fact seed and every `T_P` application. -/
def supportOp : ReasoningOp where
  conclusions := supportDerivable
  mono := fun _T₁ _T₂ hsub _l ⟨n, hn⟩ =>
    ⟨n, T_P_iter_program_mono hsub (extractFacts_program_mono hsub) n _ hn⟩

/-- Fact rules derive their own heads under `supportOp` — the
    precondition of `abduce_self_valid`, discharged concretely. -/
theorem supportOp_derives_facts (T : Theory) (l : Literal)
    (hmem : Rule.mk l [] ∈ T) : supportOp.conclusions T l :=
  ⟨0, elem_of_mem ((mem_extractFacts T l).mpr ⟨Rule.mk l [], hmem, rfl, rfl⟩)⟩

/-- `abduce_self_valid` at the concrete operator: assuming the goal itself
    always derives the goal. -/
theorem supportOp_abduce_self (T : Theory) (q : Literal) :
    supportOp.conclusions (T ++ [q].map Literal.toFact) q :=
  abduce_self_valid supportOp T q (fun _T' _l h => supportOp_derives_facts _ _ h)

end Spindle.Arith
