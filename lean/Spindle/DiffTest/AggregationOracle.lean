import Spindle.Aggregation.LoweringCorrectness
import Lean.Data.Json

/-! JSON adapter for the verified finite-domain aggregate evaluator.
Each case has rules, priorities, and domain; all four tags are decoded back to
structured user atoms. Internal guards are omitted. Errors remain per-case
results in a batch. This adapter is executable test infrastructure, not a proof
of JSON parsing. Arithmetic in the model is exact Int. -/
open Lean (Json toJson)
namespace Spindle.DiffTest.AggregationOracle
open Spindle.Aggregation

private def parseTerm (j : Json) : Except String Arith.Term := do
  if let .ok n := j.getObjValAs? Int "integer" then return .integer n
  if let .ok s := j.getObjValAs? String "symbol" then return .symbol s
  if let .ok v := j.getObjValAs? String "variable" then return .variable v
  throw "expected integer, symbol, or variable"

private def termJson : Arith.Term → Json
  | .integer n => Json.mkObj [("integer", toJson n)]
  | .symbol s => Json.mkObj [("symbol", toJson s)]
  | .variable v => Json.mkObj [("variable", toJson v)]
  | .decimal n scale => Json.mkObj [("decimal", Json.mkObj [("n", toJson n), ("scale", toJson scale)])]

private def parsePattern (j : Json) : Except String Pattern := do
  if (j.getObjVal? "mode").isOk || (j.getObjVal? "temporal").isOk then
    throw "modal and temporal patterns are outside this protocol"
  let name ← j.getObjValAs? String "name"
  let negation ← j.getObjValAs? Bool "negation"
  let args ← (← j.getObjValAs? (Array Json) "args").toList.mapM parseTerm
  return ⟨⟨name, negation, args⟩, none, false⟩

private def patternJson (p : Pattern) : Json := Json.mkObj [
  ("name", toJson p.atom.name), ("negation", toJson p.atom.negation),
  ("args", Json.arr (p.atom.args.map termJson).toArray)]

private def parseExpr : Nat → Json → Except String Expression
  | 0, _ => .error "expression nesting limit exceeded"
  | fuel + 1, j => do
    if let .ok t := j.getObjVal? "term" then return .term (← parseTerm t)
    let name ← j.getObjValAs? String "call"
    let args ← (← j.getObjValAs? (Array Json) "args").toList.mapM (parseExpr fuel)
    return .call name args

private def parseFold (j : Json) : Except String FoldExpression := do
  let resultVar ← j.getObjValAs? String "result"
  let seedJ ← j.getObjVal? "seed"
  let seed ← if seedJ == Json.null then pure none else some <$> parseExpr 64 seedJ
  let reducer ← j.getObjValAs? String "reducer"
  let extract ← parseExpr 64 (← j.getObjVal? "extract")
  let pattern ← parsePattern (← j.getObjVal? "pattern")
  let groupingVars ← j.getObjValAs? (List String) "grouping"
  return ⟨resultVar, seed, reducer, extract, pattern, groupingVars⟩

private def parseCondition (j : Json) : Except String Condition := do
  if let .ok p := j.getObjVal? "logic" then return .logic (← parsePattern p)
  if let .ok f := j.getObjVal? "fold" then return .fold (← parseFold f)
  if let .ok v := j.getObjValAs? String "bind" then
    return .bind v (← parseExpr 64 (← j.getObjVal? "value"))
  let op ← j.getObjValAs? String "compare"
  let cmp ← match op with
    | "=" => pure Arith.CmpOp.eq | "!=" => pure .ne | "<" => pure .lt
    | "<=" => pure .le | ">" => pure .gt | ">=" => pure .ge
    | _ => throw "unsupported comparison"
  return .compare cmp (← parseExpr 64 (← j.getObjVal? "left"))
    (← parseExpr 64 (← j.getObjVal? "right"))

private def parseRule (j : Json) : Except String SchemaRule := do
  let label ← j.getObjValAs? String "label"
  let kind ← match ← j.getObjValAs? String "kind" with
    | "fact" => pure RuleType.fact | "strict" => pure .strict
    | "defeasible" => pure .defeasible | "defeater" => pure .defeater
    | _ => throw "unsupported rule kind"
  let heads ← (← j.getObjValAs? (Array Json) "heads").toList.mapM parsePattern
  let body ← (← j.getObjValAs? (Array Json) "body").toList.mapM parseCondition
  match heads with
  | [] => throw "empty rule heads"
  | head :: rest => return ⟨label, kind, body, head, rest⟩

private def tagJson : ConclusionType → Json
  | .definitelyProvable => toJson "+D" | .definitelyNotProvable => toJson "-D"
  | .defeasiblyProvable => toJson "+d" | .defeasiblyNotProvable => toJson "-d"

private def evaluateCase (j : Json) : Except String Json := do
  let rules ← (← j.getObjValAs? (Array Json) "rules").toList.mapM parseRule
  let priorities ← j.getObjValAs? (List (String × String)) "priorities"
  let domain ← (← j.getObjValAs? (Array Json) "domain").toList.mapM parseTerm
  let (result, state) ← evaluateProgram (program := ⟨rules, priorities⟩) (domain := domain)
  let observations := (result.table.flatMap fun p => [p, p.complement]).flatMap fun p =>
    (state.conclusions.filter fun c => c.literal == encodeAtom result.table p).map fun c =>
      Json.mkObj [("atom", patternJson p), ("tag", tagJson c.conclusionType)]
  return Json.mkObj [("stage_count", toJson result.stageCount),
    ("conclusions", Json.arr observations.toArray)]

/-- Errors are data, so one rejected schema does not truncate a batch. -/
def processCase (j : Json) : Json :=
  match evaluateCase j with
  | .ok result => result
  | .error message => Json.mkObj [("error", toJson message)]

/-- Read one JSON case or batch and return exactly one JSON response. -/
def run : IO UInt32 := do
  let input ← (← IO.getStdin).readToEnd
  match Json.parse input with
  | .error message => IO.eprintln message; return 1
  | .ok j =>
    let output := match j.getObjValAs? (Array Json) "cases" with
      | .ok cases => Json.mkObj [("results", Json.arr (cases.map processCase))]
      | .error _ => processCase j
    IO.println output.compress
    return 0
end Spindle.DiffTest.AggregationOracle

def main : IO UInt32 := Spindle.DiffTest.AggregationOracle.run
