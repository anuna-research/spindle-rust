/-
  Spindle Term Type

  Four-variant term type for Spindle's logic language (SPEC-017).
  Models the type hierarchy INT < DECIMAL via NumericType tags,
  and cross-type equality via numeric promotion before comparison.
-/
import Spindle.Arith.Promotion

namespace Spindle.Arith

/-! ## Term type -/

/-- A term in Spindle's logic language.
    Numeric variants mirror `Value` but live at the term level;
    `symbol` and `variable` are non-numeric. -/
inductive Term where
  | symbol (s : String)
  | integer (n : Int)
  | decimal (n : Int) (scale : Nat)
  | variable (name : String)
  deriving Repr, Inhabited, BEq, DecidableEq

/-! ## Numeric type tag -/

/-- Extract the numeric type tag, if the term is numeric. -/
def Term.numericType : Term → Option NumericType
  | .integer _ => some .int
  | .decimal _ _ => some .decimal
  | _ => none

/-- Is this term numeric? -/
def Term.isNumeric : Term → Bool
  | .integer _ | .decimal _ _ => true
  | _ => false

/-! ## Conversion to/from Value -/

/-- Convert a numeric term to its corresponding Value.
    Non-numeric terms return none. -/
def Term.toValue : Term → Option Value
  | .integer n => some (.int n)
  | .decimal n s => some (.decimal n s)
  | _ => none

/-- Convert a Value back to a Term. -/
def Term.ofValue : Value → Term
  | .int n => .integer n
  | .decimal n s => .decimal n s

/-- Round-trip: ofValue . toValue is identity on numeric terms. -/
theorem Term.ofValue_toValue_integer (n : Int) :
    Term.ofValue (Term.toValue (.integer n)).get! = .integer n := rfl

theorem Term.ofValue_toValue_decimal (n : Int) (s : Nat) :
    Term.ofValue (Term.toValue (.decimal n s)).get! = .decimal n s := rfl

/-! ## Cross-type numeric equality -/

/-- Cross-type numeric equality for terms.
    Two numeric terms are equal when their promoted values agree.
    Non-numeric terms, or a numeric vs. non-numeric term, are never
    numerically equal (returns false). -/
def Term.numeric_eq (a b : Term) : Bool :=
  match a.toValue, b.toValue with
  | some va, some vb => va.numeric_eq vb
  | _, _ => false

/-! ## numeric_eq properties -/

/-- numeric_eq is reflexive on numeric terms. -/
theorem Term.numeric_eq_refl_integer (n : Int) :
    (Term.integer n).numeric_eq (Term.integer n) = true := by
  simp only [Term.numeric_eq, Term.toValue]
  exact numeric_eq_refl _

theorem Term.numeric_eq_refl_decimal (n : Int) (s : Nat) :
    (Term.decimal n s).numeric_eq (Term.decimal n s) = true := by
  simp only [Term.numeric_eq, Term.toValue]
  exact numeric_eq_refl _

/-- numeric_eq is symmetric on terms. -/
theorem Term.numeric_eq_symm (a b : Term) :
    a.numeric_eq b = b.numeric_eq a := by
  simp only [Term.numeric_eq]
  cases ha : a.toValue <;> cases hb : b.toValue <;> simp
  exact Spindle.Arith.numeric_eq_symm _ _

/-- integer n is numerically equal to decimal n 0 (cross-type). -/
theorem Term.numeric_eq_int_decimal_zero (n : Int) :
    (Term.integer n).numeric_eq (Term.decimal n 0) = true := by
  simp only [Term.numeric_eq, Term.toValue]
  exact Spindle.Arith.numeric_eq_int_decimal_zero n

/-- Non-numeric terms are never numerically equal to anything. -/
theorem Term.numeric_eq_symbol_false (s : String) (t : Term) :
    (Term.symbol s).numeric_eq t = false := by
  simp only [Term.numeric_eq, Term.toValue]

theorem Term.numeric_eq_variable_false (v : String) (t : Term) :
    (Term.variable v).numeric_eq t = false := by
  simp only [Term.numeric_eq, Term.toValue]

/-! ## Type hierarchy ordering -/

/-- A numeric term's type is "at most" another type in the hierarchy. -/
def Term.typeLeq (a b : Term) : Bool :=
  match a.numericType, b.numericType with
  | some ta, some tb => ta.lub tb == tb
  | _, _ => false

/-- INT ≤ DECIMAL in the type hierarchy. -/
theorem Term.int_leq_decimal (n : Int) (m : Int) (s : Nat) :
    Term.typeLeq (.integer n) (.decimal m s) = true := by
  simp only [Term.typeLeq, Term.numericType, NumericType.lub]; decide

end Spindle.Arith
