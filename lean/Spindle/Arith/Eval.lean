/-
  Spindle Arithmetic Evaluation

  Evaluation functions for each operator family (SPEC-017).
  - NaryOps: sum, product (associative+commutative), min/max (also idempotent)
  - BinOps: sub, div, mod, pow; division by zero returns none
  - UnaryOps: neg, abs, sqrt, ceil, floor, round
  - CmpOps: comparison after promotion to the LUB type
-/
import Spindle.Arith.Promotion

namespace Spindle.Arith

/-! ## Decimal arithmetic helpers -/

/-- Scale two decimals to a common scale (the larger one). Returns (n₁', n₂', commonScale). -/
def alignScales (n₁ : Int) (s₁ : Nat) (n₂ : Int) (s₂ : Nat) : Int × Int × Nat :=
  if s₁ ≤ s₂ then
    (n₁ * (10 ^ (s₂ - s₁) : Nat), n₂, s₂)
  else
    (n₁, n₂ * (10 ^ (s₁ - s₂) : Nat), s₁)

/-! ## Value-level binary arithmetic -/

/-- Add two values after promoting to their LUB type. -/
def Value.add (a b : Value) : Option Value :=
  let t := a.typeOf.lub b.typeOf
  match a.promote t, b.promote t with
  | .int x, .int y => some (.int (x + y))
  | .decimal x sx, .decimal y sy =>
    let (x', y', s) := alignScales x sx y sy
    some (.decimal (x' + y') s)
  | .float x, .float y => some (.float (x + y))
  | _, _ => none

/-- Multiply two values after promoting to their LUB type. -/
def Value.mul (a b : Value) : Option Value :=
  let t := a.typeOf.lub b.typeOf
  match a.promote t, b.promote t with
  | .int x, .int y => some (.int (x * y))
  | .decimal x sx, .decimal y sy => some (.decimal (x * y) (sx + sy))
  | .float x, .float y => some (.float (x * y))
  | _, _ => none

/-- Minimum of two values after promoting to their LUB type. -/
def Value.min (a b : Value) : Option Value :=
  let t := a.typeOf.lub b.typeOf
  match a.promote t, b.promote t with
  | .int x, .int y => some (.int (if x ≤ y then x else y))
  | .decimal x sx, .decimal y sy =>
    let (x', y', s) := alignScales x sx y sy
    if x' ≤ y' then some (.decimal x' s) else some (.decimal y' s)
  | .float x, .float y => some (.float (if x < y then x else y))
  | _, _ => none

/-- Maximum of two values after promoting to their LUB type. -/
def Value.max (a b : Value) : Option Value :=
  let t := a.typeOf.lub b.typeOf
  match a.promote t, b.promote t with
  | .int x, .int y => some (.int (if x ≥ y then x else y))
  | .decimal x sx, .decimal y sy =>
    let (x', y', s) := alignScales x sx y sy
    if x' ≤ y' then some (.decimal y' s) else some (.decimal x' s)
  | .float x, .float y => some (.float (if x > y then x else y))
  | _, _ => none

/-! ## N-ary operator evaluation -/

/-- Identity element for each n-ary operator. -/
def NaryArithOp.identity : NaryArithOp → Value
  | .sum => .int 0
  | .product => .int 1
  | .min => .int 0  -- no true identity; handled specially for empty lists
  | .max => .int 0

/-- Fold one value into an accumulator for an n-ary op. -/
def NaryArithOp.fold (op : NaryArithOp) (acc val : Value) : Option Value :=
  match op with
  | .sum => acc.add val
  | .product => acc.mul val
  | .min => acc.min val
  | .max => acc.max val

/-- Evaluate an n-ary operator over a list of values.
    Empty list for sum/product returns the identity element.
    Empty list for min/max returns none (no identity for these on all types). -/
def evalNary (op : NaryArithOp) (args : List Value) : Option Value :=
  match args with
  | [] =>
    match op with
    | .sum => some (.int 0)
    | .product => some (.int 1)
    | .min | .max => none
  | v :: vs => vs.foldlM (op.fold) v

/-! ## Binary operator evaluation -/

/-- Subtract two values after promoting to their LUB type. -/
def Value.sub (a b : Value) : Option Value :=
  let t := a.typeOf.lub b.typeOf
  match a.promote t, b.promote t with
  | .int x, .int y => some (.int (x - y))
  | .decimal x sx, .decimal y sy =>
    let (x', y', s) := alignScales x sx y sy
    some (.decimal (x' - y') s)
  | .float x, .float y => some (.float (x - y))
  | _, _ => none

/-- Check if a value is zero. -/
def Value.isZero : Value → Bool
  | .int 0 => true
  | .decimal 0 _ => true
  | .float f => f == 0.0
  | _ => false

/-- Divide two values. Returns none on division by zero. -/
def Value.div (a b : Value) : Option Value :=
  if b.isZero then none
  else
    let t := a.typeOf.lub b.typeOf
    match a.promote t, b.promote t with
    | .int x, .int y => some (.int (x / y))
    | .decimal x sx, .decimal y sy =>
      -- (x / 10^sx) / (y / 10^sy) = (x * 10^sy) / (y * 10^sx)
      -- Result scale = sy (we keep the divisor's scale for consistency)
      let (x', y', _) := alignScales x sx y sy
      some (.decimal (x' / y') sx)
    | .float x, .float y => some (.float (x / y))
    | _, _ => none

/-- Modulo two values. Returns none on division by zero. -/
def Value.mod (a b : Value) : Option Value :=
  if b.isZero then none
  else
    let t := a.typeOf.lub b.typeOf
    match a.promote t, b.promote t with
    | .int x, .int y => some (.int (x % y))
    | .decimal x sx, .decimal y sy =>
      let (x', y', s) := alignScales x sx y sy
      some (.decimal (x' % y') s)
    | .float x, .float y => some (.float (x - y * (x / y).floor))  -- approximate fmod
    | _, _ => none

/-- Integer power. For non-integer exponents, promotes to float. -/
def Value.pow (base exp : Value) : Option Value :=
  match exp with
  | .int (.ofNat n) =>
    match base with
    | .int b => some (.int (b ^ n))
    | .decimal b s => some (.decimal (b ^ n) (s * n))
    | .float b => some (.float (Float.pow b (Float.ofNat n)))
  | .int (.negSucc _) =>
    -- Negative exponent: promote to float
    let bf := base.promote .float
    let ef := exp.promote .float
    match bf, ef with
    | .float b, .float e => some (.float (Float.pow b e))
    | _, _ => none
  | .float e =>
    match base.promote .float with
    | .float b => some (.float (Float.pow b e))
    | _ => none
  | .decimal n s =>
    -- Promote both to float
    match base.promote .float, (Value.decimal n s).promote .float with
    | .float b, .float e => some (.float (Float.pow b e))
    | _, _ => none

/-- Evaluate a binary operator. Returns none on division by zero or type errors. -/
def evalBin (op : BinArithOp) (lhs rhs : Value) : Option Value :=
  match op with
  | .sub => lhs.sub rhs
  | .div => lhs.div rhs
  | .mod => lhs.mod rhs
  | .pow => lhs.pow rhs

/-! ## Unary operator evaluation -/

/-- Integer absolute value. -/
def Int.abs' : Int → Int
  | .ofNat n => .ofNat n
  | .negSucc n => .ofNat (n + 1)

/-- Evaluate a unary operator on a value. -/
def evalUnary (op : UnaryArithOp) (v : Value) : Option Value :=
  match op with
  | .neg =>
    match v with
    | .int n => some (.int (-n))
    | .decimal n s => some (.decimal (-n) s)
    | .float f => some (.float (-f))
  | .abs =>
    match v with
    | .int n => some (.int (Int.abs' n))
    | .decimal n s => some (.decimal (Int.abs' n) s)
    | .float f => some (.float f.abs)
  | .sqrt =>
    match v.promote .float with
    | .float f => if f ≥ 0.0 then some (.float f.sqrt) else none
    | _ => none
  | .ceil =>
    match v with
    | .int n => some (.int n)
    | .decimal n s =>
      let divisor : Int := (10 ^ s : Nat)
      let q := n / divisor
      if n % divisor == 0 then some (.int q) else some (.int (q + 1))
    | .float f => some (.float f.ceil)
  | .floor =>
    match v with
    | .int n => some (.int n)
    | .decimal n s =>
      let divisor : Int := (10 ^ s : Nat)
      some (.int (n / divisor))
    | .float f => some (.float f.floor)
  | .round =>
    match v with
    | .int n => some (.int n)
    | .decimal n s =>
      let divisor : Int := (10 ^ s : Nat)
      let q := n / divisor
      let r := (n % divisor).natAbs
      let half := (divisor.natAbs + 1) / 2
      if r ≥ half then some (.int (q + 1)) else some (.int q)
    | .float f => some (.float f.round)

/-! ## Comparison operator evaluation -/

/-- Compare two promoted values. Returns a three-way ordering. -/
def Value.compare (a b : Value) : Option Ordering :=
  let t := a.typeOf.lub b.typeOf
  match a.promote t, b.promote t with
  | .int x, .int y => some (Ord.compare x y)
  | .decimal x sx, .decimal y sy =>
    let (x', y', _) := alignScales x sx y sy
    some (Ord.compare x' y')
  | .float x, .float y =>
    if x < y then some .lt
    else if x > y then some .gt
    else if x == y then some .eq
    else none  -- NaN
  | _, _ => none

/-- Evaluate a comparison operator on two values.
    Values are promoted to their LUB type before comparison. -/
def evalCmp (op : CmpOp) (lhs rhs : Value) : Option Bool :=
  match Value.compare lhs rhs with
  | none => none
  | some ord =>
    some <| match op with
    | .eq => ord == .eq
    | .ne => ord != .eq
    | .lt => ord == .lt
    | .le => ord != .gt
    | .gt => ord == .gt
    | .ge => ord != .lt

/-! ## Properties: N-ary operators -/

/-- sum of a single element returns that element. -/
theorem evalNary_sum_singleton (v : Value) :
    evalNary .sum [v] = some v := by
  simp [evalNary, List.foldlM]

/-- product of a single element returns that element. -/
theorem evalNary_product_singleton (v : Value) :
    evalNary .product [v] = some v := by
  simp [evalNary, List.foldlM]

/-- min of a single element returns that element. -/
theorem evalNary_min_singleton (v : Value) :
    evalNary .min [v] = some v := by
  simp [evalNary, List.foldlM]

/-- max of a single element returns that element. -/
theorem evalNary_max_singleton (v : Value) :
    evalNary .max [v] = some v := by
  simp [evalNary, List.foldlM]

/-- sum of empty list is 0. -/
theorem evalNary_sum_empty : evalNary .sum [] = some (.int 0) := rfl

/-- product of empty list is 1. -/
theorem evalNary_product_empty : evalNary .product [] = some (.int 1) := rfl

/-- min of empty list is none. -/
theorem evalNary_min_empty : evalNary .min [] = none := rfl

/-- max of empty list is none. -/
theorem evalNary_max_empty : evalNary .max [] = none := rfl

/-! ## Properties: Division by zero -/

/-- Integer division by zero returns none. -/
theorem evalBin_div_zero_int (n : Int) :
    evalBin .div (.int n) (.int 0) = none := by
  simp [evalBin, Value.div, Value.isZero]

/-- Decimal division by zero returns none. -/
theorem evalBin_div_zero_decimal (n : Int) (s : Nat) :
    evalBin .div (.int n) (.decimal 0 s) = none := by
  simp [evalBin, Value.div, Value.isZero]

/-- Integer mod by zero returns none. -/
theorem evalBin_mod_zero_int (n : Int) :
    evalBin .mod (.int n) (.int 0) = none := by
  simp [evalBin, Value.mod, Value.isZero]

/-! ## Properties: Comparison after promotion -/

/-- Equal integers compare as eq. -/
theorem evalCmp_eq_int_refl (n : Int) :
    evalCmp .eq (.int n) (.int n) = some true := by
  simp [evalCmp, Value.compare, Value.promote, Value.typeOf, NumericType.lub,
        Ord.compare, compareOfLessAndEq, Int.lt_irrefl]

/-- An integer equals its decimal representation with scale 0. -/
theorem evalCmp_eq_int_decimal_zero (n : Int) :
    evalCmp .eq (.int n) (.decimal n 0) = some true := by
  simp [evalCmp, Value.compare, Value.promote, Value.typeOf, NumericType.lub,
        alignScales, Ord.compare, compareOfLessAndEq, Int.lt_irrefl]

/-! ## Properties: Unary operators -/

/-- Negation of negation is identity for integers. -/
theorem evalUnary_neg_neg_int (n : Int) :
    (evalUnary .neg (.int n)).bind (evalUnary .neg) = some (.int n) := by
  simp [evalUnary, Option.bind, Int.neg_neg]

/-- abs of a non-negative integer is identity. -/
theorem evalUnary_abs_nonneg (n : Nat) :
    evalUnary .abs (.int (.ofNat n)) = some (.int (.ofNat n)) := by
  simp [evalUnary, Int.abs']

/-- floor of an integer is identity. -/
theorem evalUnary_floor_int (n : Int) :
    evalUnary .floor (.int n) = some (.int n) := rfl

/-- ceil of an integer is identity. -/
theorem evalUnary_ceil_int (n : Int) :
    evalUnary .ceil (.int n) = some (.int n) := rfl

/-- round of an integer is identity. -/
theorem evalUnary_round_int (n : Int) :
    evalUnary .round (.int n) = some (.int n) := rfl

end Spindle.Arith
