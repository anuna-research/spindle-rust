/-
  Spindle Arithmetic Types

  Core type definitions for the arithmetic module (SPEC-017).
  Covers numeric values, arithmetic operators (n-ary, binary, unary),
  comparison operators, arithmetic expressions, and constraints.
-/

namespace Spindle.Arith

/-! ## Numeric values -/

/-- Runtime numeric value with three representations. -/
inductive Value where
  | int (n : Int)
  | decimal (n : Int) (scale : Nat)  -- fixed-point: value = n / 10^scale
  | float (val : Float)
  deriving Repr, BEq, Inhabited

/-! ## Operator enums -/

/-- N-ary arithmetic operators (associative / variadic). -/
inductive NaryArithOp where
  | sum
  | product
  | min
  | max
  deriving Repr, BEq, DecidableEq, Inhabited

/-- Binary arithmetic operators (exactly two operands). -/
inductive BinArithOp where
  | sub
  | div
  | mod
  | pow
  deriving Repr, BEq, DecidableEq, Inhabited

/-- Unary arithmetic operators (exactly one operand). -/
inductive UnaryArithOp where
  | neg
  | abs
  | sqrt
  | ceil
  | floor
  | round
  deriving Repr, BEq, DecidableEq, Inhabited

/-- Comparison operators. -/
inductive CmpOp where
  | eq
  | ne
  | lt
  | le
  | gt
  | ge
  deriving Repr, BEq, DecidableEq, Inhabited

/-! ## Expressions and constraints -/

/-- Variable identifier (de Bruijn index or named). -/
abbrev VarId := Nat

/-- Arithmetic expression AST. -/
inductive ArithExpr where
  | lit (v : Value)
  | var (id : VarId)
  | naryOp (op : NaryArithOp) (args : List ArithExpr)
  | binOp (op : BinArithOp) (lhs rhs : ArithExpr)
  | unaryOp (op : UnaryArithOp) (arg : ArithExpr)
  deriving Repr, BEq, Inhabited

/-- Arithmetic constraint appearing in rule bodies. -/
inductive ArithConstraint where
  | bind (var : VarId) (expr : ArithExpr)
  | compare (op : CmpOp) (lhs rhs : ArithExpr)
  deriving Repr, BEq, Inhabited

/-! ## Error type -/

/-- Typed arithmetic failure reasons (for diagnostics / `why_not`). -/
inductive ArithError where
  | divisionByZero
  | integerOverflow
  | decimalOverflow
  | unboundVariable (id : VarId)
  | typeMismatch (op : String) (expected got : String)
  | nonFiniteFloat
  | reciprocalOfZero
  | comparisonFailed
  deriving Repr, BEq, Inhabited

end Spindle.Arith
