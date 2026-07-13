/-
  Spindle Arithmetic Type Promotion

  Defines the type promotion lattice INT → DECIMAL,
  proves value preservation under promotion, proves transitivity,
  and defines numeric_eq as equality after mutual promotion.
-/
import Spindle.Arith.Types

namespace Spindle.Arith

/-! ## Numeric type tags -/

/-- Numeric type tag for the promotion hierarchy. -/
inductive NumericType where
  | int
  | decimal
  deriving Repr, BEq, DecidableEq, Inhabited

/-! ## Type extraction and lattice -/

/-- Extract the type tag of a value. -/
def Value.typeOf : Value → NumericType
  | .int _ => .int
  | .decimal _ _ => .decimal

/-- Least upper bound in the promotion lattice: int ≤ decimal. -/
def NumericType.lub : NumericType → NumericType → NumericType
  | .decimal, _ => .decimal
  | _, .decimal => .decimal
  | .int, .int => .int

/-! ## LUB properties -/

theorem lub_comm (a b : NumericType) : a.lub b = b.lub a := by
  cases a <;> cases b <;> rfl

theorem lub_idem (a : NumericType) : a.lub a = a := by
  cases a <;> rfl

theorem lub_assoc (a b c : NumericType) : (a.lub b).lub c = a.lub (b.lub c) := by
  cases a <;> cases b <;> cases c <;> rfl

/-! ## Promotion function -/

/-- Promote a value to a target numeric type.
    Returns the value unchanged if already at or above the target type. -/
def Value.promote : Value → NumericType → Value
  | v@(.int _), .int => v
  | .int n, .decimal => .decimal n 0
  | v@(.decimal _ _), _ => v

/-! ## Self-promotion is identity -/

/-- Promoting a value to its own type is identity. -/
theorem promote_self (v : Value) : v.promote v.typeOf = v := by
  cases v <;> rfl

/-! ## Value preservation -/

/-- Promoting an integer to decimal preserves the value structurally:
    int(n) becomes decimal(n, 0), i.e., n * 10^0 = n. -/
theorem promote_int_decimal (n : Int) :
    (Value.int n).promote .decimal = Value.decimal n 0 := rfl

/-- Concrete example: promote(3) = 3.0 (as decimal 3 with scale 0). -/
example : (Value.int 3).promote .decimal = Value.decimal 3 0 := rfl

/-- Promoting an integer to itself is identity. -/
theorem promote_int_int (n : Int) :
    (Value.int n).promote .int = Value.int n := rfl

/-- Promoting a decimal to itself is identity. -/
theorem promote_decimal_decimal (n : Int) (s : Nat) :
    (Value.decimal n s).promote .decimal = Value.decimal n s := rfl

/-! ## Transitivity -/

/-- Promoting int to int then to decimal equals promoting directly to decimal. -/
theorem promote_transitive_int_int_dec (n : Int) :
    ((Value.int n).promote .int).promote .decimal =
    (Value.int n).promote .decimal := by
  simp only [Value.promote]

/-- Promoting a decimal to decimal then to decimal is identity. -/
theorem promote_transitive_dec_dec_dec (n : Int) (s : Nat) :
    ((Value.decimal n s).promote .decimal).promote .decimal =
    (Value.decimal n s).promote .decimal := by
  simp only [Value.promote]

/-! ## Numeric equality -/

/-- Numeric equality: two values are numerically equal when they agree
    after both are promoted to their least upper bound type. -/
def Value.numeric_eq (a b : Value) : Bool :=
  let t := a.typeOf.lub b.typeOf
  (a.promote t) == (b.promote t)

/-- BEq reflexivity for Value. -/
private theorem Value.beq_refl (v : Value) : (v == v) = true := by
  cases v with
  | int n => unfold BEq.beq instBEqValue instBEqValue.beq; simp
  | decimal n s => unfold BEq.beq instBEqValue instBEqValue.beq; simp

/-- numeric_eq is reflexive. -/
theorem numeric_eq_refl (v : Value) : v.numeric_eq v = true := by
  simp only [Value.numeric_eq, lub_idem, promote_self]
  exact Value.beq_refl v

/-- BEq commutativity for Value. -/
private theorem Value.beq_comm (a b : Value) : (a == b) = (b == a) := by
  cases a with
  | int n => cases b with
    | int m =>
      unfold BEq.beq instBEqValue instBEqValue.beq; simp only [BEq.beq]
      congr 1; exact propext ⟨Eq.symm, Eq.symm⟩
    | decimal _ _ => unfold BEq.beq instBEqValue instBEqValue.beq; rfl
  | decimal n s => cases b with
    | int _ => unfold BEq.beq instBEqValue instBEqValue.beq; rfl
    | decimal m t =>
      unfold BEq.beq instBEqValue instBEqValue.beq; simp only [BEq.beq]
      congr 1 <;> (congr 1; exact propext ⟨Eq.symm, Eq.symm⟩)

/-- numeric_eq is symmetric. -/
theorem numeric_eq_symm (a b : Value) :
    a.numeric_eq b = b.numeric_eq a := by
  simp only [Value.numeric_eq]
  rw [lub_comm a.typeOf b.typeOf]
  exact Value.beq_comm _ _

/-- Integer values with the same underlying int are numerically equal
    to their decimal representation with scale 0. -/
theorem numeric_eq_int_decimal_zero (n : Int) :
    (Value.int n).numeric_eq (Value.decimal n 0) = true := by
  simp only [Value.numeric_eq, Value.typeOf, NumericType.lub, Value.promote]
  unfold BEq.beq instBEqValue instBEqValue.beq
  simp

end Spindle.Arith
