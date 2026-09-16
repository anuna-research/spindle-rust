import Spindle.Aggregation.Syntax
import Spindle.Aggregation.Fold
import Spindle.Arith.Matching

/-! Shared integer-expression and row-matching primitives of the source language.
These contain no aggregate evaluation or rule lowering. -/
namespace Spindle.Aggregation

private def exprVars : Expression → List String
  | .term (.variable v) => [v]
  | .term _ => []
  | .call _ args => args.flatMap exprVars

def outerVars (r : SchemaRule) : List String :=
  (r.heads.flatMap (fun p => p.atom.vars) ++ r.body.flatMap (fun c => match c with
    | .logic p => p.atom.vars
    | .bind v e => v :: exprVars e
    | .compare _ a b => exprVars a ++ exprVars b
    | .fold f => f.resultVar :: f.groupingVars ++
        (f.seed.toList.flatMap exprVars) ++
        ((exprVars f.extract).filter (fun v => !f.pattern.atom.vars.contains v)))).dedup


def evalExpr (σ : Arith.Substitution) : Expression → Except String Int
  | .term t => match σ.applyTerm t with
    | .integer value => .ok value
    | _ => .error "integer expression is unbound or has an unsupported type"
  | .call name args => do
    let values ← args.mapM (evalExpr σ)
    match name, values with
    | "+", values => return values.foldl (· + ·) 0
    | "*", values => return values.foldl (· * ·) 1
    | "-", [a, b] => return a - b
    | "min", a :: rest => return rest.foldl min a
    | "max", a :: rest => return rest.foldl max a
    | _, _ => .error "unsupported expression function or arity"

def reducerNamed (name : String) : Except String (Reducer Int) :=
  if name = "+" || name = "sum" then .ok sum
  else if name = "min" then .ok minimum
  else if name = "max" then .ok maximum
  else .error "unsupported aggregate reducer"

def matchRow (σ : Arith.Substitution) (pattern row : Pattern) : Option Arith.Substitution :=
  if pattern.atom.name = row.atom.name ∧ pattern.atom.negation = row.atom.negation ∧
      pattern.modeName = row.modeName ∧ pattern.modeNegated = row.modeNegated then
    Arith.matchTerms σ pattern.atom.args row.atom.args
  else none

def compareInts (op : Arith.CmpOp) (a b : Int) : Bool :=
  match op with
  | .eq => a == b | .ne => a != b | .lt => a < b
  | .le => a ≤ b | .gt => a > b | .ge => a ≥ b


end Spindle.Aggregation
