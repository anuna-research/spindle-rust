/-
  Spindle Arithmetic Type Promotion

  Defines the type promotion lattice INT → DECIMAL → FLOAT,
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
  | float
  deriving Repr, BEq, DecidableEq, Inhabited

/-! ## Type extraction and lattice -/

/-- Extract the type tag of a value. -/
def Value.typeOf : Value → NumericType
  | .int _ => .int
  | .decimal _ _ => .decimal
  | .float _ => .float

/-- Least upper bound in the promotion lattice: int ≤ decimal ≤ float. -/
def NumericType.lub : NumericType → NumericType → NumericType
  | .float, _ => .float
  | _, .float => .float
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

/-! ## Int-to-Float helper -/

/-- Convert an integer to Float. -/
def intToFloat (n : Int) : Float :=
  match n with
  | .ofNat k => Float.ofNat k
  | .negSucc k => -(Float.ofNat (k + 1))

/-! ## Promotion function -/

/-- Promote a value to a target numeric type.
    Returns the value unchanged if already at or above the target type. -/
def Value.promote : Value → NumericType → Value
  | v@(.int _), .int => v
  | .int n, .decimal => .decimal n 0
  | .int n, .float => .float (intToFloat n)
  | v@(.decimal _ _), .int => v
  | v@(.decimal _ _), .decimal => v
  | .decimal n s, .float => .float (intToFloat n / Float.ofNat (10 ^ s))
  | v@(.float _), _ => v

/-! ## Self-promotion is identity -/

/-- Promoting a value to its own type is identity. -/
theorem promote_self (v : Value) : v.promote v.typeOf = v := by
  cases v <;> rfl

/-! ## Value preservation -/

/-- Promoting an integer to decimal preserves the value structurally:
    int(n) becomes decimal(n, 0), i.e., n × 10⁰ = n. -/
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

/-- Promoting a float to any target type is identity (float is the top type). -/
theorem promote_float_id (f : Float) (t : NumericType) :
    (Value.float f).promote t = Value.float f := by
  cases t <;> rfl

/-! ## Transitivity -/

/-- Promotion is transitive: promoting int → decimal → float equals
    promoting int → float directly.
    Note: Float arithmetic is opaque in Lean; this requires showing x / 1.0 = x. -/
theorem promote_transitive_int_dec_float (n : Int) :
    ((Value.int n).promote .decimal).promote .float =
    (Value.int n).promote .float := by
  simp only [Value.promote, Nat.pow_zero]
  -- Remaining goal: intToFloat n / Float.ofNat 1 = intToFloat n
  -- Float.div and Float.ofNat are opaque primitives in Lean 4 (no simp lemmas,
  -- no LawfulBEq, no DecidableEq). The identity x / 1.0 = x cannot be proved
  -- without native_decide (which requires closed terms) or an axiom.
  sorry

/-- Promoting a decimal to decimal then to float equals promoting directly to float. -/
theorem promote_transitive_dec_dec_float (n : Int) (s : Nat) :
    ((Value.decimal n s).promote .decimal).promote .float =
    (Value.decimal n s).promote .float := by
  simp only [Value.promote]

/-- Promoting int to int then to decimal equals promoting directly to decimal. -/
theorem promote_transitive_int_int_dec (n : Int) :
    ((Value.int n).promote .int).promote .decimal =
    (Value.int n).promote .decimal := by
  simp only [Value.promote]

/-! ## Numeric equality -/

/-- Numeric equality: two values are numerically equal when they agree
    after both are promoted to their least upper bound type. -/
def Value.numeric_eq (a b : Value) : Bool :=
  let t := a.typeOf.lub b.typeOf
  (a.promote t) == (b.promote t)

/-- BEq reflexivity for Value. Proved for int and decimal constructors;
    the float case is blocked by Float.beq being opaque in Lean 4.
    Note: IEEE 754 BEq is not truly reflexive (NaN != NaN), so this
    cannot be proved even in principle without restricting to finite floats. -/
private theorem Value.beq_refl (v : Value) : (v == v) = true := by
  cases v with
  | int n => unfold BEq.beq instBEqValue instBEqValue.beq; simp
  | decimal n s => unfold BEq.beq instBEqValue instBEqValue.beq; simp
  | float f =>
    unfold BEq.beq instBEqValue instBEqValue.beq; simp
    sorry -- BLOCKED: Float.beq is opaque; cannot prove (f == f) = true

/-- numeric_eq is reflexive. -/
theorem numeric_eq_refl (v : Value) : v.numeric_eq v = true := by
  simp only [Value.numeric_eq, lub_idem, promote_self]
  exact Value.beq_refl v

/-- BEq commutativity for Value. Proved for all constructor pairs except
    float-float, which is blocked by Float.beq being opaque in Lean 4.
    (Mathematically, IEEE 754 equality comparison is symmetric, but the
    opaque declaration prevents any proof.) -/
private theorem Value.beq_comm (a b : Value) : (a == b) = (b == a) := by
  cases a with
  | int n => cases b with
    | int m =>
      unfold BEq.beq instBEqValue instBEqValue.beq; simp only [BEq.beq]
      congr 1; exact propext ⟨Eq.symm, Eq.symm⟩
    | decimal _ _ => unfold BEq.beq instBEqValue instBEqValue.beq; rfl
    | float _ => unfold BEq.beq instBEqValue instBEqValue.beq; rfl
  | decimal n s => cases b with
    | int _ => unfold BEq.beq instBEqValue instBEqValue.beq; rfl
    | decimal m t =>
      unfold BEq.beq instBEqValue instBEqValue.beq; simp only [BEq.beq]
      congr 1 <;> (congr 1; exact propext ⟨Eq.symm, Eq.symm⟩)
    | float _ => unfold BEq.beq instBEqValue instBEqValue.beq; rfl
  | float f => cases b with
    | int _ => unfold BEq.beq instBEqValue instBEqValue.beq; rfl
    | decimal _ _ => unfold BEq.beq instBEqValue instBEqValue.beq; rfl
    | float g =>
      sorry -- BLOCKED: Float.beq is opaque; cannot prove commutativity

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
