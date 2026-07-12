/-
  Spindle Arithmetic Evaluation

  Evaluation functions for each operator family (SPEC-017).
  - NaryOps: sum, product (associative+commutative), min/max (also
    idempotent), div (true division; 1-arg = reciprocal)
  - BinOps: sub, div (integer floor division), mod, pow; division by zero
    returns none
  - UnaryOps: neg, abs, sqrt, ceil, floor, round
  - CmpOps: comparison after promotion to the LUB type

  ## Numeric model (faithful to the Rust engine)

  Rust evaluates integers as `i64` with CHECKED arithmetic and decimals
  as `rust_decimal::Decimal`: a 96-bit signed mantissa at scale 0–28.
  Results outside those bounds are errors, or lose fractional digits with
  banker's rounding (round half to even) when the scale can absorb the
  overflow. This module models exactly that: `fitInt` enforces the i64
  range, `fitDecimal` the 96-bit/scale-28 range with half-even rounding,
  and true division (`Value.divTrue`) rounds half-even at the largest
  representable scale — all verified empirically against
  rust_decimal 1.40 (see crates/spindle-core/tests/lean_arith_oracle_difftest.rs).

  NOT modeled: IEEE-754 floats (`NumericValue::Float`). The differential
  test generator never produces float inputs; float behavior is engine-
  only and out of the verified scope (documented in lean/DIVERGENCES.md).
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

/-! ## Representability bounds (Rust engine semantics) -/

/-- Smallest Rust `i64`. -/
def intMin : Int := -(2 ^ 63)

/-- Largest Rust `i64`. -/
def intMax : Int := 2 ^ 63 - 1

/-- Largest rust_decimal mantissa: 2^96 − 1. -/
def maxMantissa : Nat := 2 ^ 96 - 1

/-- Largest rust_decimal scale. -/
def maxScale : Nat := 28

/-- Wrap an integer result with Rust's checked-i64 semantics:
    out-of-range results are overflow errors. -/
def fitInt (n : Int) : Option Value :=
  if intMin ≤ n && n ≤ intMax then some (.int n) else none

/-- Round `n / d` (`d ≠ 0`) to the nearest integer, ties to even —
    rust_decimal's rounding when fractional digits are dropped
    (verified against rust_decimal 1.40: 2/3 → …67, 5·10⁻²⁹ → 0). -/
def roundHalfEvenDiv (n d : Int) : Int :=
  let sign : Int := if (n < 0) == (d < 0) then 1 else -1
  let na := n.natAbs
  let da := d.natAbs
  let q := na / da
  let r := na % da
  let q' := if 2 * r > da then q + 1
            else if 2 * r < da then q
            else if q % 2 == 0 then q else q + 1
  sign * (q' : Int)

/-- Fit a raw (mantissa, scale) result into rust_decimal's representable
    range: pick the largest target scale `≤ min s maxScale` at which the
    half-even-rounded mantissa fits in 96 bits (a SINGLE rounding from the
    exact value, as rust_decimal does); `none` when even the integer part
    cannot fit (DecimalOverflow). -/
def fitDecimal (n : Int) (s : Nat) : Option Value :=
  go (Nat.min s maxScale)
where
  go : Nat → Option Value
    | 0 =>
      let m := roundHalfEvenDiv n ((10 : Int) ^ s)
      if m.natAbs ≤ maxMantissa then some (.decimal m 0) else none
    | t + 1 =>
      let m := roundHalfEvenDiv n ((10 : Int) ^ (s - (t + 1)))
      if m.natAbs ≤ maxMantissa then some (.decimal m (t + 1)) else go t

/-! ## Value-level binary arithmetic -/

/-- Add two values after promoting to their LUB type. -/
def Value.add (a b : Value) : Option Value :=
  let t := a.typeOf.lub b.typeOf
  match a.promote t, b.promote t with
  | .int x, .int y => fitInt (x + y)
  | .decimal x sx, .decimal y sy =>
    let (x', y', s) := alignScales x sx y sy
    fitDecimal (x' + y') s
  | _, _ => none

/-- Multiply two values after promoting to their LUB type. -/
def Value.mul (a b : Value) : Option Value :=
  let t := a.typeOf.lub b.typeOf
  match a.promote t, b.promote t with
  | .int x, .int y => fitInt (x * y)
  | .decimal x sx, .decimal y sy => fitDecimal (x * y) (sx + sy)
  | _, _ => none

/-- Minimum of two values after promoting to their LUB type. -/
def Value.min (a b : Value) : Option Value :=
  let t := a.typeOf.lub b.typeOf
  match a.promote t, b.promote t with
  | .int x, .int y => some (.int (if x ≤ y then x else y))
  | .decimal x sx, .decimal y sy =>
    let (x', y', s) := alignScales x sx y sy
    if x' ≤ y' then some (.decimal x' s) else some (.decimal y' s)
  | _, _ => none

/-- Maximum of two values after promoting to their LUB type. -/
def Value.max (a b : Value) : Option Value :=
  let t := a.typeOf.lub b.typeOf
  match a.promote t, b.promote t with
  | .int x, .int y => some (.int (if x ≥ y then x else y))
  | .decimal x sx, .decimal y sy =>
    let (x', y', s) := alignScales x sx y sy
    if x' ≤ y' then some (.decimal y' s) else some (.decimal x' s)
  | _, _ => none

/-! ## N-ary operator evaluation -/

/-- Check if a value is zero. -/
def Value.isZero : Value → Bool
  | .int 0 => true
  | .decimal 0 _ => true
  | _ => false

/-- True division `/` (Rust `NaryArithOp::Div`, `div_values`):
    Integer/Integer promotes to DECIMAL division (REQ-005), and decimal
    division mirrors rust_decimal `checked_div` — the quotient is rounded
    half-even at the largest scale ≤ 28 whose mantissa fits in 96 bits;
    `none` on division by zero or when the integer part overflows.

    The previous model aligned scales and performed INTEGER division
    (1.00 / 2.00 = 0.00) — that was wrong; Rust yields 0.5. -/
def Value.divTrue (a b : Value) : Option Value :=
  if b.isZero then none
  else
    let (nx, sx) := match a with | .int n => (n, 0) | .decimal n s => (n, s)
    let (ny, sy) := match b with | .int n => (n, 0) | .decimal n s => (n, s)
    -- (nx / 10^sx) / (ny / 10^sy) = (nx·10^sy) / (ny·10^sx)
    go (nx * (10 : Int) ^ sy) (ny * (10 : Int) ^ sx) maxScale
where
  /-- Largest scale ≤ fuel at which the half-even-rounded quotient
      mantissa fits 96 bits; a single rounding per candidate scale. -/
  go (num den : Int) : Nat → Option Value
    | 0 =>
      let m := roundHalfEvenDiv num den
      if m.natAbs ≤ maxMantissa then some (.decimal m 0) else none
    | s + 1 =>
      let m := roundHalfEvenDiv (num * (10 : Int) ^ (s + 1)) den
      if m.natAbs ≤ maxMantissa then some (.decimal m (s + 1)) else go num den s

/-- Reciprocal (1/x): Rust `reciprocal` — Integer/Decimal input yields a
    Decimal; `none` on zero. -/
def Value.recip (v : Value) : Option Value :=
  Value.divTrue (.int 1) v

/-- Identity element for each n-ary operator. -/
def NaryArithOp.identity : NaryArithOp → Value
  | .sum => .int 0
  | .product => .int 1
  | .min => .int 0  -- no true identity; handled specially for empty lists
  | .max => .int 0
  | .div => .int 1  -- no true identity; empty/unary lists handled specially

/-- Fold one value into an accumulator for an n-ary op. -/
def NaryArithOp.fold (op : NaryArithOp) (acc val : Value) : Option Value :=
  match op with
  | .sum => acc.add val
  | .product => acc.mul val
  | .min => acc.min val
  | .max => acc.max val
  | .div => acc.divTrue val

/-- Evaluate an n-ary operator over a list of values.
    Empty list for sum/product returns the identity element.
    Empty list for min/max/div returns none (no identity on all types).
    A 1-element div is the reciprocal (Rust `NaryArithOp::Div` arity-1). -/
def evalNary (op : NaryArithOp) (args : List Value) : Option Value :=
  match op, args with
  | .sum, [] => some (.int 0)
  | .product, [] => some (.int 1)
  | .min, [] | .max, [] | .div, [] => none
  | .div, [v] => v.recip
  | op, v :: vs => vs.foldlM (op.fold) v

/-! ## Binary operator evaluation -/

/-- Subtract two values after promoting to their LUB type. -/
def Value.sub (a b : Value) : Option Value :=
  let t := a.typeOf.lub b.typeOf
  match a.promote t, b.promote t with
  | .int x, .int y => fitInt (x - y)
  | .decimal x sx, .decimal y sy =>
    let (x', y', s) := alignScales x sx y sy
    fitDecimal (x' - y') s
  | _, _ => none

/-- Floor division `div` — Rust `BinArithOp::IDiv`: INTEGERS ONLY (any
    decimal operand is a TypeMismatch in Rust, `none` here), rounding
    toward negative infinity, with the `i64::MIN / -1` overflow guard.

    The previous model promoted decimals and divided aligned mantissas;
    Rust rejects non-integer operands outright. -/
def Value.div (a b : Value) : Option Value :=
  match a, b with
  | .int x, .int y => if y == 0 then none else fitInt (Int.fdiv x y)
  | _, _ => none

/-- Floor remainder `mod` — Rust `BinArithOp::Rem`: INTEGERS ONLY, result
    carries the divisor's sign (floor semantics), with the same
    `i64::MIN / -1` guard Rust applies via its `checked_div` probe. -/
def Value.mod (a b : Value) : Option Value :=
  match a, b with
  | .int x, .int y =>
    if y == 0 then none
    else if x == intMin && y == -1 then none
    else some (.int (Int.fmod x y))
  | _, _ => none

/-- Exponentiation, mirroring Rust `eval_pow` (float cases excluded from
    the model):
    - int ^ non-negative int: checked i64 power;
    - int ^ negative int: checked power then decimal reciprocal
      (`DivisionByZero` on base 0);
    - decimal ^ non-negative int: exact power fitted to rust_decimal's
      range (Rust squares with per-step fitting; the difftest generator
      only exercises integer bases, where the two agree);
    - decimal ^ negative int: fitted power then reciprocal;
    - decimal exponents: not modeled (Rust uses the transcendental
      `checked_powd` for fractional exponents). -/
def Value.pow (base exp : Value) : Option Value :=
  match exp with
  | .int (.ofNat n) =>
    match base with
    | .int b => fitInt (b ^ n)
    | .decimal b s => fitDecimal (b ^ n) (s * n)
  | .int (.negSucc m) =>
    match base with
    | .int b =>
      if b == 0 then none
      else (fitInt (b ^ (m + 1))).bind fun _ =>
        Value.divTrue.go 1 (b ^ (m + 1)) maxScale
    | .decimal b s =>
      (fitDecimal (b ^ (m + 1)) (s * (m + 1))).bind fun v => (Value.int 1).divTrue v
  | .decimal _ _ => none

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

/-- Evaluate a unary operator on a value. Integer negation/abs are
    checked (Rust `checked_neg`/`checked_abs`: overflow at `i64::MIN`). -/
def evalUnary (op : UnaryArithOp) (v : Value) : Option Value :=
  match op with
  | .neg =>
    match v with
    | .int n => fitInt (-n)
    | .decimal n s => some (.decimal (-n) s)
  | .abs =>
    match v with
    | .int n => fitInt (Int.abs' n)
    | .decimal n s => some (.decimal (Int.abs' n) s)
  | .sqrt => none  -- sqrt not supported without float
  | .ceil =>
    match v with
    | .int n => some (.int n)
    | .decimal n s =>
      let divisor : Int := (10 ^ s : Nat)
      let q := n / divisor
      if n % divisor == 0 then some (.int q) else some (.int (q + 1))
  | .floor =>
    match v with
    | .int n => some (.int n)
    | .decimal n s =>
      let divisor : Int := (10 ^ s : Nat)
      some (.int (n / divisor))
  | .round =>
    match v with
    | .int n => some (.int n)
    | .decimal n s =>
      let divisor : Int := (10 ^ s : Nat)
      let q := n / divisor
      let r := (n % divisor).natAbs
      let half := (divisor.natAbs + 1) / 2
      if r ≥ half then some (.int (q + 1)) else some (.int q)

/-! ## Comparison operator evaluation -/

/-- Compare two promoted values. Returns a three-way ordering. -/
def Value.compare (a b : Value) : Option Ordering :=
  let t := a.typeOf.lub b.typeOf
  match a.promote t, b.promote t with
  | .int x, .int y => some (Ord.compare x y)
  | .decimal x sx, .decimal y sy =>
    let (x', y', _) := alignScales x sx y sy
    some (Ord.compare x' y')
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

/-- div of empty list is none (Rust: `/` demands at least one operand). -/
theorem evalNary_div_empty : evalNary .div [] = none := rfl

/-! ## Properties: Division -/

/-- Integer floor division by zero returns none. -/
theorem evalBin_div_zero_int (n : Int) :
    evalBin .div (.int n) (.int 0) = none := by
  simp [evalBin, Value.div]

/-- Floor division rejects decimal operands (Rust IDiv is integer-only). -/
theorem evalBin_div_decimal_type_error (n : Int) (m : Int) (s : Nat) :
    evalBin .div (.int n) (.decimal m s) = none := rfl

/-- Integer mod by zero returns none. -/
theorem evalBin_mod_zero_int (n : Int) :
    evalBin .mod (.int n) (.int 0) = none := by
  simp [evalBin, Value.mod]

/-- True division by zero returns none. -/
theorem evalNary_div_zero (n : Int) :
    evalNary .div [.int n, .int 0] = none := rfl

/-- The regression that motivated this model: 1.00 / 2.00 evaluates to
    0.5 (as rust_decimal does), NOT to the scale-aligned integer
    division 0.00 the previous model computed. -/
theorem divTrue_one_half :
    Value.divTrue (.decimal 100 2) (.decimal 200 2) = some (.decimal 5000000000000000000000000000 28) := by
  rfl

/-- Integer/Integer true division promotes to decimal: 1 / 2 = 0.5. -/
theorem divTrue_int_promotes :
    Value.divTrue (.int 1) (.int 2) = some (.decimal 5000000000000000000000000000 28) := by
  rfl

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

/-- In-range integers pass the checked-i64 wrapper unchanged. -/
theorem fitInt_eq_some {n : Int} (h₁ : intMin ≤ n) (h₂ : n ≤ intMax) :
    fitInt n = some (.int n) := by
  simp [fitInt, h₁, h₂]

/-- Negation of negation is identity for in-range integers. The bounds
    are required by the checked-i64 model: Rust's `checked_neg` overflows
    at `i64::MIN`. -/
theorem evalUnary_neg_neg_int (n : Int) (hlo : intMin < n) (hhi : n ≤ intMax) :
    (evalUnary .neg (.int n)).bind (evalUnary .neg) = some (.int n) := by
  have hmin : intMin = -9223372036854775808 := rfl
  have hmax : intMax = 9223372036854775807 := rfl
  have h1 : evalUnary .neg (.int n) = some (.int (-n)) :=
    fitInt_eq_some (by omega) (by omega)
  rw [h1]
  show evalUnary .neg (.int (-n)) = some (.int n)
  have h2 : evalUnary .neg (.int (-n)) = some (.int (-(-n))) :=
    fitInt_eq_some (by omega) (by omega)
  rw [h2, Int.neg_neg]

/-- abs of a non-negative in-range integer is identity. -/
theorem evalUnary_abs_nonneg (n : Nat) (h : (n : Int) ≤ intMax) :
    evalUnary .abs (.int (.ofNat n)) = some (.int (.ofNat n)) := by
  have hmin : intMin = -9223372036854775808 := rfl
  show fitInt (Int.abs' (.ofNat n)) = some (.int (.ofNat n))
  have habs : Int.abs' (.ofNat n) = .ofNat n := rfl
  rw [habs]
  have hcast : Int.ofNat n = (n : Int) := rfl
  exact fitInt_eq_some (by omega) h

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
