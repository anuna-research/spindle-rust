/-
  Spindle Arithmetic Oracle

  JSON-based oracle for differential testing of arithmetic evaluation.
  Reads a JSON test case from stdin, evaluates using the Lean arithmetic
  functions, and writes the result as JSON to stdout.

  JSON wire format:
  - Input:  { "expr": <ArithExpr>, "env": [{"id": N, "value": <Value>}] }
  - Output: { "ok": <Value> } | { "error": "<message>" }

  Value format:
    { "int": N }
    { "decimal": { "n": N, "scale": S } }
    { "float": F }

  ArithExpr format:
    { "lit": <Value> }
    { "var": N }
    { "naryOp": { "op": "sum"|"product"|"min"|"max", "args": [<ArithExpr>] } }
    { "binOp": { "op": "sub"|"div"|"mod"|"pow", "lhs": <ArithExpr>, "rhs": <ArithExpr> } }
    { "unaryOp": { "op": "neg"|"abs"|"sqrt"|"ceil"|"floor"|"round", "arg": <ArithExpr> } }

  Constraint format (optional "constraint" field instead of "expr"):
    { "constraint": <ArithConstraint>, "env": [...] }
    { "bind": { "var": N, "expr": <ArithExpr> } }
    { "compare": { "op": "eq"|"ne"|"lt"|"le"|"gt"|"ge", "lhs": <ArithExpr>, "rhs": <ArithExpr> } }

  Constraint output:
    { "satisfied": [{"id": N, "value": <Value>}] }
    { "unsatisfied": true }
    { "error": "<message>" }
-/
import Spindle.Arith.Constraint
import Lean.Data.Json

open Lean (Json JsonNumber ToJson FromJson)

namespace Spindle.DiffTest.ArithOracle

open Spindle.Arith

/-! ## JSON helpers -/

private def jsonInt (n : Int) : Json := Json.num (JsonNumber.fromInt n)
private def jsonNat (n : Nat) : Json := Json.num (JsonNumber.fromNat n)

/-! ## Value JSON -/

private def valueToJson : Value → Json
  | .int n => Json.mkObj [("int", jsonInt n)]
  | .decimal n s =>
    Json.mkObj [("decimal", Json.mkObj [
      ("n", jsonInt n),
      ("scale", jsonNat s)
    ])]
  | .float f => Json.mkObj [("float", Json.str (toString f))]

private def parseValue (j : Json) : Except String Value := do
  -- Try int
  if let some n := j.getObjValAs? Int "int" |>.toOption then
    return .int n
  -- Try decimal
  if let some obj := j.getObjVal? "decimal" |>.toOption then
    let n ← obj.getObjValAs? Int "n"
    let scale ← obj.getObjValAs? Nat "scale"
    return .decimal n scale
  -- Try float (as number or string)
  if let some f := j.getObjValAs? Float "float" |>.toOption then
    return .float f
  .error s!"invalid Value JSON: {j}"

/-! ## Operator JSON -/

private def parseNaryOp (s : String) : Except String NaryArithOp :=
  match s with
  | "sum" => .ok .sum
  | "product" => .ok .product
  | "min" => .ok .min
  | "max" => .ok .max
  | _ => .error s!"unknown NaryArithOp: {s}"

private def parseBinOp (s : String) : Except String BinArithOp :=
  match s with
  | "sub" => .ok .sub
  | "div" => .ok .div
  | "mod" => .ok .mod
  | "pow" => .ok .pow
  | _ => .error s!"unknown BinArithOp: {s}"

private def parseUnaryOp (s : String) : Except String UnaryArithOp :=
  match s with
  | "neg" => .ok .neg
  | "abs" => .ok .abs
  | "sqrt" => .ok .sqrt
  | "ceil" => .ok .ceil
  | "floor" => .ok .floor
  | "round" => .ok .round
  | _ => .error s!"unknown UnaryArithOp: {s}"

private def parseCmpOp (s : String) : Except String CmpOp :=
  match s with
  | "eq" => .ok .eq
  | "ne" => .ok .ne
  | "lt" => .ok .lt
  | "le" => .ok .le
  | "gt" => .ok .gt
  | "ge" => .ok .ge
  | _ => .error s!"unknown CmpOp: {s}"

/-! ## ArithExpr JSON -/

private partial def parseExpr (j : Json) : Except String ArithExpr := do
  -- lit
  if let some litJ := j.getObjVal? "lit" |>.toOption then
    let v ← parseValue litJ
    return .lit v
  -- var
  if let some n := j.getObjValAs? Nat "var" |>.toOption then
    return .var n
  -- naryOp
  if let some obj := j.getObjVal? "naryOp" |>.toOption then
    let opStr ← obj.getObjValAs? String "op"
    let op ← parseNaryOp opStr
    let argsJ ← obj.getObjValAs? (Array Json) "args"
    let args ← argsJ.toList.mapM parseExpr
    return .naryOp op args
  -- binOp
  if let some obj := j.getObjVal? "binOp" |>.toOption then
    let opStr ← obj.getObjValAs? String "op"
    let op ← parseBinOp opStr
    let lhs ← parseExpr (← obj.getObjVal? "lhs")
    let rhs ← parseExpr (← obj.getObjVal? "rhs")
    return .binOp op lhs rhs
  -- unaryOp
  if let some obj := j.getObjVal? "unaryOp" |>.toOption then
    let opStr ← obj.getObjValAs? String "op"
    let op ← parseUnaryOp opStr
    let arg ← parseExpr (← obj.getObjVal? "arg")
    return .unaryOp op arg
  .error s!"invalid ArithExpr JSON: {j}"

/-! ## ArithConstraint JSON -/

private partial def parseConstraint (j : Json) : Except String ArithConstraint := do
  -- bind
  if let some obj := j.getObjVal? "bind" |>.toOption then
    let var ← obj.getObjValAs? Nat "var"
    let expr ← parseExpr (← obj.getObjVal? "expr")
    return .bind var expr
  -- compare
  if let some obj := j.getObjVal? "compare" |>.toOption then
    let opStr ← obj.getObjValAs? String "op"
    let op ← parseCmpOp opStr
    let lhs ← parseExpr (← obj.getObjVal? "lhs")
    let rhs ← parseExpr (← obj.getObjVal? "rhs")
    return .compare op lhs rhs
  .error s!"invalid ArithConstraint JSON: {j}"

/-! ## ValueEnv JSON -/

private def parseEnvBindings (j : Json) : Except String (List (VarId × Value)) := do
  let arr ← j.getArr?
  arr.toList.mapM fun entry => do
    let id ← entry.getObjValAs? Nat "id"
    let valJ ← entry.getObjVal? "value"
    let v ← parseValue valJ
    return (id, v)

private def buildEnv (bindings : List (VarId × Value)) : ValueEnv :=
  fun id => bindings.find? (fun (k, _) => k == id) |>.map (·.2)

/-! ## Main oracle logic -/

/-- Evaluate an expression test case. -/
private def evalExprCase (j : Json) : Except String Json := do
  let exprJ ← j.getObjVal? "expr"
  let envJ ← j.getObjVal? "env"
  let expr ← parseExpr exprJ
  let bindings ← parseEnvBindings envJ
  let env := buildEnv bindings
  match expr.eval env with
  | some v => .ok (Json.mkObj [("ok", valueToJson v)])
  | none => .ok (Json.mkObj [("error", Json.str "evaluation failed")])

/-- Evaluate a constraint test case. -/
private def evalConstraintCase (j : Json) : Except String Json := do
  let constraintJ ← j.getObjVal? "constraint"
  let envJ ← j.getObjVal? "env"
  let constraint ← parseConstraint constraintJ
  let bindings ← parseEnvBindings envJ
  let env := buildEnv bindings
  match constraint.eval env with
  | .satisfied env' =>
    -- Collect all known variable IDs and their values in the new environment
    let maxId := bindings.foldl (fun acc (id, _) => Nat.max acc id) 0
    -- Check binding IDs up to maxId + some extra for newly bound vars
    let ids := List.range (maxId + 10)
    let newBindings := ids.filterMap fun id =>
      match env' id with
      | some v => some (id, v)
      | none => none
    let envArr := newBindings.map fun (id, v) =>
      Json.mkObj [("id", jsonNat id), ("value", valueToJson v)]
    .ok (Json.mkObj [("satisfied", Json.arr envArr.toArray)])
  | .unsatisfied =>
    .ok (Json.mkObj [("unsatisfied", Json.bool true)])
  | .error e =>
    .ok (Json.mkObj [("error", Json.str (toString (repr e)))])

/-- Process a single JSON test case (expression or constraint). -/
def processCase (j : Json) : Except String Json := do
  -- Check if it's a constraint case or expression case
  if (j.getObjVal? "constraint").toOption.isSome then
    evalConstraintCase j
  else if (j.getObjVal? "expr").toOption.isSome then
    evalExprCase j
  else
    .error "JSON must have either 'expr' or 'constraint' field"

/-- Process a batch of test cases. -/
def processBatch (j : Json) : Except String Json := do
  let cases ← j.getObjValAs? (Array Json) "cases"
  let results ← cases.toList.mapM processCase
  .ok (Json.mkObj [("results", Json.arr results.toArray)])

/-- Main entry point: read JSON from stdin, evaluate, write result to stdout. -/
def main (_args : List String) : IO UInt32 := do
  let stdin ← IO.getStdin
  let input ← stdin.readToEnd
  let json ← match Json.parse input with
    | .ok j => pure j
    | .error e => do
      IO.eprintln s!"JSON parse error: {e}"
      return 1

  -- Check for batch mode
  let result ← if (json.getObjVal? "cases").toOption.isSome then
    match processBatch json with
    | .ok r => pure r
    | .error e => do
      IO.eprintln s!"batch processing error: {e}"
      return 1
  else
    match processCase json with
    | .ok r => pure r
    | .error e => do
      IO.eprintln s!"processing error: {e}"
      return 1

  IO.println (toString result)
  return 0

end Spindle.DiffTest.ArithOracle

/-- Entry point for the ArithOracle executable. -/
def main (args : List String) : IO UInt32 :=
  Spindle.DiffTest.ArithOracle.main args
