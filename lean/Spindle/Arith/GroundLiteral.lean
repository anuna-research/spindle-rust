/-
  Spindle Ground Literal

  Defines literal type with predicate arguments, substitution application,
  and groundness (SPEC-017). Proves that applying a complete substitution
  to a literal produces a ground literal.
-/
import Spindle.Arith.Substitution

namespace Spindle.Arith

/-! ## Literal type -/

/-- A literal in Spindle's logic language.
    Consists of a predicate name, a negation flag, and a list of term arguments.
    Mirrors the Rust `Literal` struct's core fields relevant to grounding. -/
structure Literal where
  /-- Predicate name (e.g., "bird", "parent"). -/
  name : String
  /-- Whether this literal is negated. -/
  negation : Bool
  /-- Predicate arguments (e.g., for parent(alice, bob)). -/
  args : List Term
  deriving Repr, Inhabited, BEq

/-! ## Groundness -/

/-- A literal is ground iff all its predicate arguments are ground. -/
def Literal.isGround (l : Literal) : Bool :=
  l.args.all Term.isGround

/-- A literal with no arguments is trivially ground. -/
theorem Literal.isGround_nil (name : String) (neg : Bool) :
    (Literal.mk name neg []).isGround = true := rfl

/-- A single-argument literal is ground iff the argument is ground. -/
theorem Literal.isGround_singleton (name : String) (neg : Bool) (t : Term) :
    (Literal.mk name neg [t]).isGround = t.isGround := by
  simp only [Literal.isGround, List.all_cons, List.all_nil, Bool.and_true]

/-! ## Variables in a literal -/

/-- Collect all variable names occurring in a literal's arguments. -/
def Literal.vars (l : Literal) : List String :=
  l.args.filterMap fun
    | .variable v => some v
    | _ => none

/-! ## Substitution application -/

/-- Apply a substitution to a literal, replacing variables in predicate arguments. -/
def Literal.applySubst (σ : Substitution) (l : Literal) : Literal :=
  { l with args := σ.applyTerms l.args }

/-- Applying a substitution preserves the literal's name. -/
theorem Literal.applySubst_name (σ : Substitution) (l : Literal) :
    (l.applySubst σ).name = l.name := rfl

/-- Applying a substitution preserves the literal's negation. -/
theorem Literal.applySubst_negation (σ : Substitution) (l : Literal) :
    (l.applySubst σ).negation = l.negation := rfl

/-- Applying the empty substitution is identity. -/
theorem Literal.applySubst_empty (l : Literal) :
    l.applySubst Substitution.empty = l := by
  simp only [Literal.applySubst, Substitution.applyTerms]
  cases l with
  | mk name neg args =>
    simp only [Literal.mk.injEq, true_and]
    induction args with
    | nil => rfl
    | cons t ts ih =>
      simp only [List.map_cons, List.cons.injEq]
      exact ⟨Substitution.applyTerm_empty t, ih⟩

/-- Applying a substitution to a literal with no arguments is identity. -/
theorem Literal.applySubst_nil (σ : Substitution) (name : String) (neg : Bool) :
    (Literal.mk name neg []).applySubst σ = Literal.mk name neg [] := rfl

/-! ## Completeness and ground application -/

/-- A substitution is complete for a term if applying it produces a ground term. -/
def Substitution.groundsTerm (σ : Substitution) (t : Term) : Bool :=
  (σ.applyTerm t).isGround

/-- A substitution is complete for a literal if applying it grounds every argument. -/
def Substitution.completeLiteral (σ : Substitution) (l : Literal) : Bool :=
  l.args.all σ.groundsTerm

/-- A substitution that maps a variable to a ground term grounds that variable. -/
theorem Substitution.groundsTerm_variable (σ : Substitution) (v : String) (t : Term)
    (hlook : σ.lookup v = some t) (hgnd : t.isGround = true) :
    σ.groundsTerm (.variable v) = true := by
  simp only [Substitution.groundsTerm, Substitution.applyTerm, hlook, hgnd]

/-- Non-variable terms are always grounded by any substitution. -/
theorem Substitution.groundsTerm_nonvar_symbol (σ : Substitution) (s : String) :
    σ.groundsTerm (.symbol s) = true := rfl

theorem Substitution.groundsTerm_nonvar_integer (σ : Substitution) (n : Int) :
    σ.groundsTerm (.integer n) = true := rfl

theorem Substitution.groundsTerm_nonvar_decimal (σ : Substitution) (n : Int) (s : Nat) :
    σ.groundsTerm (.decimal n s) = true := rfl

theorem Substitution.groundsTerm_nonvar_float (σ : Substitution) (f : Float) :
    σ.groundsTerm (.finiteFloat f) = true := rfl

/-! ## Main theorem: complete substitution produces ground literal -/

/-- Helper: applying a substitution to a list preserves the all-ground property
    when the substitution grounds every term in the list. -/
private theorem applyTerms_all_ground (σ : Substitution) (ts : List Term)
    (h : ts.all σ.groundsTerm = true) :
    (σ.applyTerms ts).all Term.isGround = true := by
  induction ts with
  | nil => rfl
  | cons t ts ih =>
    simp only [Substitution.applyTerms, List.map_cons, List.all_cons,
               Bool.and_eq_true] at h ⊢
    exact ⟨h.1, ih h.2⟩

/-- **Main theorem**: applying a complete substitution to a literal produces
    a ground literal. A substitution is "complete" for a literal when it maps
    every variable in the literal's arguments to a ground term. -/
theorem Literal.applySubst_ground (σ : Substitution) (l : Literal)
    (hcomplete : σ.completeLiteral l = true) :
    (l.applySubst σ).isGround = true := by
  simp only [Literal.isGround, Literal.applySubst, Substitution.completeLiteral] at *
  exact applyTerms_all_ground σ l.args hcomplete

/-! ## Auxiliary lemmas for the covering corollary -/

/-- A term in a ground substitution's range is ground. -/
private theorem ground_term_of_ground_subst_mem (pairs : List (String × Term))
    (hall : pairs.all (fun p => p.2.isGround) = true)
    (t : Term) (v : String) (hmem : (v, t) ∈ pairs) :
    t.isGround = true := by
  induction pairs with
  | nil => exact absurd hmem (List.not_mem_nil _)
  | cons p ps ih =>
    simp only [List.all_cons, Bool.and_eq_true] at hall
    cases List.mem_cons.mp hmem with
    | inl heq =>
      have := congrArg Prod.snd heq
      simp at this
      rw [← this]
      exact hall.1
    | inr hmem => exact ih hall.2 hmem

/-- If `lookup` succeeds, the pair is in the association list. -/
private theorem lookup_some_mem {pairs : List (String × Term)} {v : String} {t : Term}
    (h : pairs.lookup v = some t) : (v, t) ∈ pairs := by
  induction pairs with
  | nil => simp [List.lookup] at h
  | cons p ps ih =>
    simp only [List.lookup] at h
    split at h
    · next heq =>
      injection h with h
      have := beq_iff_eq.mp heq
      exact List.mem_cons.mpr (Or.inl (Prod.ext this h))
    · exact List.mem_cons.mpr (Or.inr (ih h))

/-- **Corollary**: a ground substitution that covers all variables in a literal
    is complete for that literal. -/
theorem Substitution.complete_of_ground_covering (σ : Substitution) (l : Literal)
    (hgnd : σ.isGround = true)
    (hcov : ∀ v, v ∈ l.vars → σ.inDomain v = true) :
    σ.completeLiteral l = true := by
  simp only [Substitution.completeLiteral]
  induction l.args with
  | nil => rfl
  | cons t ts ih =>
    simp only [List.all_cons, Bool.and_eq_true]
    constructor
    · cases t with
      | variable v =>
        simp only [Substitution.groundsTerm, Substitution.applyTerm]
        have hdom : σ.inDomain v = true := by
          apply hcov
          simp only [Literal.vars, List.filterMap_cons]
          exact List.mem_cons_self v _
        simp only [Substitution.inDomain, Option.isSome_some, Option.isSome_none] at hdom
        split
        · next t' hlook =>
          exact ground_term_of_ground_subst_mem σ.map hgnd t' v
            (lookup_some_mem hlook)
        · next hnone =>
          simp only [Substitution.lookup] at hnone
          simp_all [Option.isSome]
      | symbol _ => rfl
      | integer _ => rfl
      | decimal _ _ => rfl
      | finiteFloat _ => rfl
    · apply ih
      intro v hv
      apply hcov
      simp only [Literal.vars, List.filterMap_cons]
      cases t with
      | variable w => exact List.mem_cons_of_mem _ hv
      | symbol _ => exact hv
      | integer _ => exact hv
      | decimal _ _ => exact hv
      | finiteFloat _ => exact hv

end Spindle.Arith
