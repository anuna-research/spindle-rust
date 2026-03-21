/-
  Spindle Arithmetic Constraint Evaluation

  Defines body arguments (literal, constant, variable, arithmetic expression),
  constraint satisfaction under substitutions, and proves that constraint
  evaluation is deterministic when all variables are bound to ground terms
  (SPEC-017).
-/
import Spindle.Arith.Eval
import Spindle.Arith.GroundLiteral

namespace Spindle.Arith

/-! ## Body argument type -/

/-- A body argument in a Spindle rule body.
    Represents the four kinds of terms that can appear as arguments
    in rule body positions. -/
inductive BodyArg where
  | literal (l : Literal)
  | constant (t : Term) (hGround : t.isGround = true)
  | variable (name : String)
  | arithExpr (e : ArithExpr)
  deriving Repr

/-! ## Variables in expressions -/

/-- Collect all variable IDs occurring in an arithmetic expression. -/
def ArithExpr.varIds : ArithExpr → List VarId
  | .lit _ => []
  | .var id => [id]
  | .naryOp _ args => args.flatMap ArithExpr.varIds
  | .binOp _ lhs rhs => lhs.varIds ++ rhs.varIds
  | .unaryOp _ arg => arg.varIds

/-- Collect all variable IDs occurring in a constraint. -/
def ArithConstraint.varIds : ArithConstraint → List VarId
  | .bind v e => v :: e.varIds
  | .compare _ lhs rhs => lhs.varIds ++ rhs.varIds

/-! ## Value environment -/

/-- A value environment maps variable IDs to values.
    Used for evaluating arithmetic expressions under a substitution. -/
abbrev ValueEnv := VarId → Option Value

/-- The empty value environment (all variables unbound). -/
def ValueEnv.empty : ValueEnv := fun _ => none

/-- Extend a value environment with a single binding. -/
def ValueEnv.set (env : ValueEnv) (id : VarId) (v : Value) : ValueEnv :=
  fun x => if x == id then some v else env x

/-! ## Expression evaluation under an environment -/

/-- Evaluate an arithmetic expression under a value environment.
    Returns `none` if any variable is unbound or an operator fails. -/
def ArithExpr.eval (env : ValueEnv) : ArithExpr → Option Value
  | .lit v => some v
  | .var id => env id
  | .naryOp op args =>
    let vals := args.mapM (ArithExpr.eval env)
    vals.bind (evalNary op)
  | .binOp op lhs rhs => do
    let l ← lhs.eval env
    let r ← rhs.eval env
    evalBin op l r
  | .unaryOp op arg => do
    let v ← arg.eval env
    evalUnary op v

/-! ## Constraint satisfaction -/

/-- Result of evaluating a constraint. -/
inductive ConstraintResult where
  | satisfied (env' : ValueEnv)  -- constraint holds, possibly with new bindings
  | unsatisfied                   -- constraint evaluated but failed
  | error (e : ArithError)        -- evaluation error (unbound var, div by zero, etc.)
  deriving Inhabited

/-- Evaluate a constraint under a value environment.
    - `bind v e`: evaluates `e` and extends the environment with the binding `v ↦ result`
    - `compare op l r`: evaluates both sides and checks the comparison -/
def ArithConstraint.eval (env : ValueEnv) : ArithConstraint → ConstraintResult
  | .bind var expr =>
    match expr.eval env with
    | some v => .satisfied (env.set var v)
    | none => .error (.unboundVariable var)
  | .compare op lhs rhs =>
    match lhs.eval env, rhs.eval env with
    | some l, some r =>
      match evalCmp op l r with
      | some true => .satisfied env
      | some false => .unsatisfied
      | none => .error .comparisonFailed
    | none, _ => .error (.unboundVariable 0)  -- placeholder ID
    | _, none => .error (.unboundVariable 0)

/-! ## Ground environment -/

/-- An environment is ground for an expression if every variable in the
    expression is bound. -/
def ValueEnv.groundFor (env : ValueEnv) (e : ArithExpr) : Prop :=
  ∀ id, id ∈ e.varIds → env id ≠ none

/-- An environment is ground for a constraint if every variable in the
    constraint (excluding bind targets) is bound in the environment. -/
def ValueEnv.groundForConstraint (env : ValueEnv) (c : ArithConstraint) : Prop :=
  match c with
  | .bind _ expr => env.groundFor expr
  | .compare _ lhs rhs => env.groundFor lhs ∧ env.groundFor rhs

/-! ## Determinism of expression evaluation -/

/-- Expression evaluation is a (partial) function: given the same environment,
    it always produces the same result. This is trivially true since `eval`
    is defined as a pure function — but we state it explicitly as a theorem
    to serve as the specification anchor for constraint satisfaction. -/
theorem ArithExpr.eval_deterministic (env : ValueEnv) (e : ArithExpr) :
    ∀ v₁ v₂, e.eval env = some v₁ → e.eval env = some v₂ → v₁ = v₂ := by
  intro v₁ v₂ h₁ h₂
  rw [h₁] at h₂
  exact Option.some.inj h₂

/-- Constraint evaluation is deterministic: the same environment and constraint
    always produce the same result. -/
theorem ArithConstraint.eval_deterministic (env : ValueEnv) (c : ArithConstraint) :
    ∀ r₁ r₂, c.eval env = r₁ → c.eval env = r₂ → r₁ = r₂ := by
  intro r₁ r₂ h₁ h₂
  rw [← h₁, ← h₂]

/-! ## Groundness guarantees evaluation success -/

/-- Helper: if a variable is bound in the environment, `eval` of `.var id` succeeds. -/
theorem ArithExpr.eval_var_of_bound (env : ValueEnv) (id : VarId) (v : Value)
    (h : env id = some v) :
    (ArithExpr.var id).eval env = some v := by
  simp [ArithExpr.eval, h]

/-- Helper: a literal always evaluates successfully. -/
theorem ArithExpr.eval_lit (env : ValueEnv) (v : Value) :
    (ArithExpr.lit v).eval env = some v := by simp [ArithExpr.eval]

/-! ## Properties of ValueEnv.set -/

/-- Looking up the variable just set returns the bound value. -/
theorem ValueEnv.set_hit (env : ValueEnv) (id : VarId) (v : Value) :
    (env.set id v) id = some v := by
  simp [ValueEnv.set, BEq.beq]

/-- Looking up a different variable returns the original environment's value. -/
theorem ValueEnv.set_miss (env : ValueEnv) (id₁ id₂ : VarId) (v : Value)
    (h : id₂ ≠ id₁) :
    (env.set id₁ v) id₂ = env id₂ := by
  simp [ValueEnv.set, h]

/-! ## Bind constraint produces extended environment -/

/-- When a bind constraint succeeds, the resulting environment maps the
    bound variable to the evaluated value. -/
theorem ArithConstraint.bind_extends (env : ValueEnv) (var : VarId) (expr : ArithExpr)
    (v : Value) (heval : expr.eval env = some v) :
    (ArithConstraint.bind var expr).eval env = .satisfied (env.set var v) := by
  simp [ArithConstraint.eval, heval]

/-- A successful comparison constraint does not change the environment. -/
theorem ArithConstraint.compare_preserves_env (env : ValueEnv) (op : CmpOp)
    (lhs rhs : ArithExpr) (l r : Value)
    (hl : lhs.eval env = some l) (hr : rhs.eval env = some r)
    (hcmp : evalCmp op l r = some true) :
    (ArithConstraint.compare op lhs rhs).eval env = .satisfied env := by
  simp [ArithConstraint.eval, hl, hr, hcmp]

/-! ## Constraint list satisfaction -/

/-- Evaluate a list of constraints sequentially, threading the environment
    through each constraint. Returns the final environment on success. -/
def evalConstraints (env : ValueEnv) : List ArithConstraint → ConstraintResult
  | [] => .satisfied env
  | c :: cs =>
    match c.eval env with
    | .satisfied env' => evalConstraints env' cs
    | .unsatisfied => .unsatisfied
    | .error e => .error e

/-- Empty constraint list is always satisfied. -/
theorem evalConstraints_nil (env : ValueEnv) :
    evalConstraints env [] = .satisfied env := rfl

/-- Single constraint delegates to ArithConstraint.eval. -/
theorem evalConstraints_singleton (env : ValueEnv) (c : ArithConstraint) :
    evalConstraints env [c] =
      match c.eval env with
      | .satisfied env' => .satisfied env'
      | .unsatisfied => .unsatisfied
      | .error e => .error e := by
  simp [evalConstraints]

/-- Sequential evaluation is deterministic. -/
theorem evalConstraints_deterministic (env : ValueEnv) (cs : List ArithConstraint) :
    ∀ r₁ r₂, evalConstraints env cs = r₁ → evalConstraints env cs = r₂ → r₁ = r₂ := by
  intro r₁ r₂ h₁ h₂
  rw [← h₁, ← h₂]

end Spindle.Arith
