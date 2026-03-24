/-
  SpindleLean.Basic
  Core types: Literal with strong negation, Mode (deontic operators)

  Strong negation (~p) is represented as a Bool field on Literal,
  NOT as Lean's logical negation. This matches defeasible logic
  where p and ~p are distinct, complementary atoms.
-/

/-- Deontic modal operators -/
inductive Mode where
  | obligation  -- [O]
  | permission  -- [P]
  | forbidden   -- [F]
  deriving DecidableEq, Repr, BEq, Hashable

/-- A literal in defeasible logic with strong negation -/
structure Literal where
  name : String
  negated : Bool := false
  mode : Option Mode := none
  deriving DecidableEq, Repr, BEq, Hashable

namespace Literal

/-- Create a positive literal -/
def pos (name : String) : Literal := ⟨name, false, none⟩

/-- Create a negated literal -/
def neg (name : String) : Literal := ⟨name, true, none⟩

/-- Complement: flip the negation -/
def complement (l : Literal) : Literal :=
  { l with negated := !l.negated }

theorem complement_involutive (l : Literal) :
    l.complement.complement = l := by
  simp [complement, Bool.not_not]

theorem complement_ne_self (l : Literal) :
    l.complement ≠ l := by
  intro h
  have := congrArg Literal.negated h
  simp [complement] at this

end Literal

instance : LawfulBEq Mode where
  eq_of_beq {a b} h := by
    cases a <;> cases b <;> (first | rfl | (simp only [BEq.beq] at h; exact absurd h (by decide)))
  rfl {a} := by cases a <;> decide

instance : LawfulBEq Literal where
  eq_of_beq {a b} h := by
    cases a; cases b
    simp only [BEq.beq, instBEqLiteral.beq, Bool.and_eq_true, decide_eq_true_eq] at h
    obtain ⟨hname, hneg, hmode⟩ := h
    subst hname; subst hneg
    have := @eq_of_beq (Option Mode) _ _ _ _ hmode
    subst this; rfl
  rfl {a} := by
    cases a
    simp only [BEq.beq, instBEqLiteral.beq, decide_true, Bool.true_and]
    have : ∀ (m : Option Mode), (Option.instBEq.beq m m) = true := by
      intro m; cases m with
      | none => decide
      | some v => cases v <;> decide
    exact this _

instance : Inhabited Literal := ⟨Literal.pos ""⟩
