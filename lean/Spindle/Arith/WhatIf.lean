/-
  Spindle What-If Specification

  Formalizes the what_if operator: given a theory T and hypothetical facts F,
  compute reason(T ∪ F) and return the delta (new conclusions minus original).

  Main results:
  - `whatIf_empty`: what_if(T, ∅) = ∅
  - `whatIf_mono`: what_if is monotone in F (more hypotheticals → more conclusions)
-/
import Spindle.Arith.GroundRule

namespace Spindle.Arith

/-! ## Theory -/

/-- A theory is a list of rules. -/
abbrev Theory := List Rule

/-! ## Converting hypothetical literals to fact rules -/

/-- Convert a literal to a fact (rule with empty body).
    Mirrors `Rule::fact` in Rust's `what_if.rs`. -/
def Literal.toFact (l : Literal) : Rule :=
  Rule.mk l []

/-- Converting to a fact preserves the literal as the head. -/
theorem Literal.toFact_head (l : Literal) :
    (Literal.toFact l).head = l := rfl

/-- A fact rule has no body. -/
theorem Literal.toFact_body (l : Literal) :
    (Literal.toFact l).body = [] := rfl

/-! ## Abstract monotone reasoning operator -/

/-- An abstract reasoning operator maps a theory to the set of literals
    it can derive. Modeled as a predicate `conclusions T l` meaning
    "literal l is derivable from theory T".

    The key requirement is **monotonicity**: adding more rules to a theory
    can only increase the set of derivable conclusions. This holds for
    Spindle's forward-chaining `reason` function. -/
structure ReasoningOp where
  /-- The conclusions derivable from a theory. -/
  conclusions : Theory → Literal → Prop
  /-- Monotonicity: if every rule in T₁ is also in T₂,
      then every conclusion of T₁ is also a conclusion of T₂. -/
  mono : ∀ T₁ T₂ : Theory, (∀ r, r ∈ T₁ → r ∈ T₂) →
    ∀ l, conclusions T₁ l → conclusions T₂ l

/-! ## What-If operator -/

/-- The what-if predicate: literal `l` is a what-if conclusion of theory `T`
    with hypothetical facts `F` iff `l` is derivable from T ∪ F (as facts)
    but not derivable from T alone.

    This captures the "delta" — the genuinely new conclusions enabled by
    the hypothetical assumptions. Mirrors the `new_conclusions` field in
    Rust's `WhatIfResult`. -/
def whatIfConclusion (R : ReasoningOp) (T : Theory) (F : List Literal)
    (l : Literal) : Prop :=
  R.conclusions (T ++ F.map Literal.toFact) l ∧ ¬ R.conclusions T l

/-! ## Theorem 1: what_if(T, ∅) = ∅ -/

/-- **Empty hypotheticals produce no new conclusions.**
    Adding zero hypothetical facts to a theory cannot change the conclusions,
    so the delta is empty.

    Formally: for any reasoning operator R, theory T, and literal l,
    `whatIfConclusion R T [] l` is false. -/
theorem whatIf_empty (R : ReasoningOp) (T : Theory) (l : Literal) :
    ¬ whatIfConclusion R T [] l := by
  intro ⟨hderiv, hnotbase⟩
  apply hnotbase
  -- T ++ [].map toFact = T ++ [] = T, so the derivation from T ∪ ∅ is from T
  simp only [List.map_nil, List.append_nil] at hderiv
  exact hderiv

/-! ## Theorem 2: Monotonicity in hypotheticals -/

/-- Augmenting T with F maps each rule in T to T ++ F.map toFact. -/
private theorem mem_augmented_of_sub {T : Theory} {F F' : List Literal}
    (hsub : ∀ l, l ∈ F → l ∈ F')
    (r : Rule) (hr : r ∈ T ++ F.map Literal.toFact) :
    r ∈ T ++ F'.map Literal.toFact := by
  simp only [List.mem_append, List.mem_map] at hr ⊢
  rcases hr with h | ⟨l, hlF, rfl⟩
  · exact Or.inl h
  · exact Or.inr ⟨l, hsub l hlF, rfl⟩

/-- **What-if is monotone in the hypothetical facts.**
    If F ⊆ F' (every hypothetical in F also appears in F'), then every
    what-if conclusion under F is also a what-if conclusion under F'.

    Proof sketch: if l ∈ reason(T ∪ F) \ reason(T) and F ⊆ F', then
    T ∪ F ⊆ T ∪ F', so by monotonicity of reason, l ∈ reason(T ∪ F'),
    and l ∉ reason(T) still holds, giving l ∈ reason(T ∪ F') \ reason(T). -/
theorem whatIf_mono (R : ReasoningOp) (T : Theory) (F F' : List Literal)
    (hsub : ∀ l, l ∈ F → l ∈ F') (l : Literal)
    (h : whatIfConclusion R T F l) :
    whatIfConclusion R T F' l := by
  obtain ⟨hderiv, hnotbase⟩ := h
  exact ⟨R.mono _ _ (mem_augmented_of_sub hsub) l hderiv, hnotbase⟩

/-! ## Corollaries -/

/-- Adding a single hypothetical: if l is a what-if conclusion under {f},
    it is also a what-if conclusion under any F containing f. -/
theorem whatIf_singleton_sub (R : ReasoningOp) (T : Theory) (f : Literal)
    (F : List Literal) (hf : f ∈ F) (l : Literal)
    (h : whatIfConclusion R T [f] l) :
    whatIfConclusion R T F l :=
  whatIf_mono R T [f] F (fun l' hl' => by simp at hl'; rw [hl']; exact hf) l h

/-- What-if conclusions are always derivable from the augmented theory. -/
theorem whatIf_derivable (R : ReasoningOp) (T : Theory) (F : List Literal)
    (l : Literal) (h : whatIfConclusion R T F l) :
    R.conclusions (T ++ F.map Literal.toFact) l :=
  h.1

/-- What-if conclusions are never derivable from the original theory. -/
theorem whatIf_not_baseline (R : ReasoningOp) (T : Theory) (F : List Literal)
    (l : Literal) (h : whatIfConclusion R T F l) :
    ¬ R.conclusions T l :=
  h.2

/-- If a literal is already derivable from T, it cannot be a what-if conclusion
    (for any set of hypotheticals). -/
theorem whatIf_not_of_baseline (R : ReasoningOp) (T : Theory) (F : List Literal)
    (l : Literal) (hbase : R.conclusions T l) :
    ¬ whatIfConclusion R T F l :=
  fun ⟨_, hnotbase⟩ => hnotbase hbase

/-- Transitivity of subset for what-if: if F₁ ⊆ F₂ ⊆ F₃,
    then what-if conclusions under F₁ are also under F₃. -/
theorem whatIf_mono_trans (R : ReasoningOp) (T : Theory)
    (F₁ F₂ F₃ : List Literal)
    (h₁₂ : ∀ l, l ∈ F₁ → l ∈ F₂) (h₂₃ : ∀ l, l ∈ F₂ → l ∈ F₃)
    (l : Literal) (h : whatIfConclusion R T F₁ l) :
    whatIfConclusion R T F₃ l :=
  whatIf_mono R T F₁ F₃ (fun l hl => h₂₃ l (h₁₂ l hl)) l h

end Spindle.Arith
