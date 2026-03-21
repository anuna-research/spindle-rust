/-
  Spindle Term Type

  Five-variant term type for Spindle's logic language (SPEC-017).
  Models the type hierarchy INT < DECIMAL < FLOAT via NumericType tags,
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
  | finiteFloat (val : Float)
  | variable (name : String)
  deriving Repr, Inhabited

/-! ## Numeric type tag -/

/-- Extract the numeric type tag, if the term is numeric. -/
def Term.numericType : Term → Option NumericType
  | .integer _ => some .int
  | .decimal _ _ => some .decimal
  | .finiteFloat _ => some .float
  | _ => none

/-- Is this term numeric? -/
def Term.isNumeric : Term → Bool
  | .integer _ | .decimal _ _ | .finiteFloat _ => true
  | _ => false

/-! ## Conversion to/from Value -/

/-- Convert a numeric term to its corresponding Value.
    Non-numeric terms return none. -/
def Term.toValue : Term → Option Value
  | .integer n => some (.int n)
  | .decimal n s => some (.decimal n s)
  | .finiteFloat f => some (.float f)
  | _ => none

/-- Convert a Value back to a Term. -/
def Term.ofValue : Value → Term
  | .int n => .integer n
  | .decimal n s => .decimal n s
  | .float f => .finiteFloat f

/-- Round-trip: ofValue ∘ toValue is identity on numeric terms. -/
theorem Term.ofValue_toValue_integer (n : Int) :
    Term.ofValue (Term.toValue (.integer n)).get! = .integer n := rfl

theorem Term.ofValue_toValue_decimal (n : Int) (s : Nat) :
    Term.ofValue (Term.toValue (.decimal n s)).get! = .decimal n s := rfl

theorem Term.ofValue_toValue_float (f : Float) :
    Term.ofValue (Term.toValue (.finiteFloat f)).get! = .finiteFloat f := rfl

/-! ## BEq instance (structural equality) -/

/-- Structural equality for Terms.
    Two terms are equal iff they have the same constructor and same fields. -/
instance : BEq Term where
  beq
    | .symbol a, .symbol b => a == b
    | .integer a, .integer b => a == b
    | .decimal a1 a2, .decimal b1 b2 => a1 == b1 && a2 == b2
    | .finiteFloat a, .finiteFloat b => a == b
    | .variable a, .variable b => a == b
    | _, _ => false

/-! ## DecidableEq instance -/

/-- Decidable equality for Terms. Float equality is delegated to BEq
    (IEEE 754 semantics), so the float case uses the opaque beq. -/
instance : DecidableEq Term := fun a b =>
  match a, b with
  | .symbol s₁, .symbol s₂ =>
    if h : s₁ = s₂ then isTrue (congrArg Term.symbol h)
    else isFalse (fun h' => by cases h'; exact h rfl)
  | .integer n₁, .integer n₂ =>
    if h : n₁ = n₂ then isTrue (congrArg Term.integer h)
    else isFalse (fun h' => by cases h'; exact h rfl)
  | .decimal n₁ s₁, .decimal n₂ s₂ =>
    if hn : n₁ = n₂ then
      if hs : s₁ = s₂ then isTrue (by rw [hn, hs])
      else isFalse (fun h' => by cases h'; exact hs rfl)
    else isFalse (fun h' => by cases h'; exact hn rfl)
  | .finiteFloat f₁, .finiteFloat f₂ =>
    if h : f₁ = f₂ then isTrue (congrArg Term.finiteFloat h)
    else isFalse (fun h' => by cases h'; exact h rfl)
  | .variable v₁, .variable v₂ =>
    if h : v₁ = v₂ then isTrue (congrArg Term.variable h)
    else isFalse (fun h' => by cases h'; exact h rfl)
  -- Cross-constructor cases are all False
  | .symbol _, .integer _ => isFalse (fun h => by cases h)
  | .symbol _, .decimal _ _ => isFalse (fun h => by cases h)
  | .symbol _, .finiteFloat _ => isFalse (fun h => by cases h)
  | .symbol _, .variable _ => isFalse (fun h => by cases h)
  | .integer _, .symbol _ => isFalse (fun h => by cases h)
  | .integer _, .decimal _ _ => isFalse (fun h => by cases h)
  | .integer _, .finiteFloat _ => isFalse (fun h => by cases h)
  | .integer _, .variable _ => isFalse (fun h => by cases h)
  | .decimal _ _, .symbol _ => isFalse (fun h => by cases h)
  | .decimal _ _, .integer _ => isFalse (fun h => by cases h)
  | .decimal _ _, .finiteFloat _ => isFalse (fun h => by cases h)
  | .decimal _ _, .variable _ => isFalse (fun h => by cases h)
  | .finiteFloat _, .symbol _ => isFalse (fun h => by cases h)
  | .finiteFloat _, .integer _ => isFalse (fun h => by cases h)
  | .finiteFloat _, .decimal _ _ => isFalse (fun h => by cases h)
  | .finiteFloat _, .variable _ => isFalse (fun h => by cases h)
  | .variable _, .symbol _ => isFalse (fun h => by cases h)
  | .variable _, .integer _ => isFalse (fun h => by cases h)
  | .variable _, .decimal _ _ => isFalse (fun h => by cases h)
  | .variable _, .finiteFloat _ => isFalse (fun h => by cases h)

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

theorem Term.numeric_eq_refl_float (f : Float) :
    (Term.finiteFloat f).numeric_eq (Term.finiteFloat f) = true := by
  simp only [Term.numeric_eq, Term.toValue]
  exact numeric_eq_refl _

/-- numeric_eq is symmetric on terms. -/
theorem Term.numeric_eq_symm (a b : Term) :
    a.numeric_eq b = b.numeric_eq a := by
  simp only [Term.numeric_eq]
  cases ha : a.toValue <;> cases hb : b.toValue <;> simp
  exact numeric_eq_symm _ _

/-- integer n is numerically equal to decimal n 0 (cross-type). -/
theorem Term.numeric_eq_int_decimal_zero (n : Int) :
    (Term.integer n).numeric_eq (Term.decimal n 0) = true := by
  simp only [Term.numeric_eq, Term.toValue]
  exact numeric_eq_int_decimal_zero n

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
  simp only [Term.typeLeq, Term.numericType, NumericType.lub]

/-- INT ≤ FLOAT in the type hierarchy. -/
theorem Term.int_leq_float (n : Int) (f : Float) :
    Term.typeLeq (.integer n) (.finiteFloat f) = true := by
  simp only [Term.typeLeq, Term.numericType, NumericType.lub]

/-- DECIMAL ≤ FLOAT in the type hierarchy. -/
theorem Term.decimal_leq_float (n : Int) (s : Nat) (f : Float) :
    Term.typeLeq (.decimal n s) (.finiteFloat f) = true := by
  simp only [Term.typeLeq, Term.numericType, NumericType.lub]

end Spindle.Arith
