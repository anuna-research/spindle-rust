/-
  Spindle Grounding Oracle

  JSON-based oracle for differential testing of grounding.
  Reads a JSON test case from stdin containing rules with variables
  and a domain, computes ground instances using the Lean grounding
  functions, and writes the results as JSON to stdout.

  JSON wire format:
  - Input:
    {
      "rules": [{"head": <Literal>, "body": [<Literal>]}],
      "domain": [<Term>]
    }
  - Output:
    {
      "results": [
        {"rule_index": N, "ground_instances": [{"head": <Literal>, "body": [<Literal>]}]}
      ]
    }

  Term format:
    { "symbol": "s" }
    { "integer": N }
    { "decimal": { "n": N, "scale": S } }
    { "float": "f" }
    { "variable": "v" }

  Literal format:
    { "name": "s", "negation": true/false, "args": [<Term>] }

  Batch mode:
    { "cases": [<Input>, ...] }
    → { "results": [<Output>, ...] }
-/
import Spindle.Arith.GroundRule
import Lean.Data.Json

open Lean (Json JsonNumber)

namespace Spindle.DiffTest.GroundingOracle

open Spindle.Arith

/-! ## JSON helpers -/

private def jsonInt (n : Int) : Json := Json.num (JsonNumber.fromInt n)
private def jsonNat (n : Nat) : Json := Json.num (JsonNumber.fromNat n)

/-! ## Term JSON -/

private def termToJson : Term → Json
  | .symbol s => Json.mkObj [("symbol", Json.str s)]
  | .integer n => Json.mkObj [("integer", jsonInt n)]
  | .decimal n s =>
    Json.mkObj [("decimal", Json.mkObj [
      ("n", jsonInt n),
      ("scale", jsonNat s)
    ])]
  | .finiteFloat f => Json.mkObj [("float", Json.str (toString f))]
  | .variable v => Json.mkObj [("variable", Json.str v)]

private def parseTerm (j : Json) : Except String Term := do
  -- symbol
  if let some s := j.getObjValAs? String "symbol" |>.toOption then
    return .symbol s
  -- integer
  if let some n := j.getObjValAs? Int "integer" |>.toOption then
    return .integer n
  -- decimal
  if let some obj := j.getObjVal? "decimal" |>.toOption then
    let n ← obj.getObjValAs? Int "n"
    let scale ← obj.getObjValAs? Nat "scale"
    return .decimal n scale
  -- float (as number or string)
  if let some f := j.getObjValAs? Float "float" |>.toOption then
    return .finiteFloat f
  -- variable
  if let some v := j.getObjValAs? String "variable" |>.toOption then
    return .variable v
  .error s!"invalid Term JSON: {j}"

/-! ## Literal JSON -/

private def literalToJson (l : Literal) : Json :=
  Json.mkObj [
    ("name", Json.str l.name),
    ("negation", Json.bool l.negation),
    ("args", Json.arr (l.args.map termToJson).toArray)
  ]

private def parseLiteral (j : Json) : Except String Literal := do
  let name ← j.getObjValAs? String "name"
  let negation ← j.getObjValAs? Bool "negation"
  let argsJ ← j.getObjValAs? (Array Json) "args"
  let args ← argsJ.toList.mapM parseTerm
  return { name, negation, args }

/-! ## Rule JSON -/

private def ruleToJson (r : Rule) : Json :=
  Json.mkObj [
    ("head", literalToJson r.head),
    ("body", Json.arr (r.body.map literalToJson).toArray)
  ]

private def parseRule (j : Json) : Except String Rule := do
  let headJ ← j.getObjVal? "head"
  let head ← parseLiteral headJ
  let bodyJ ← j.getObjValAs? (Array Json) "body"
  let body ← bodyJ.toList.mapM parseLiteral
  return { head, body }

/-! ## Domain JSON -/

private def parseDomain (j : Json) : Except String (List Term) := do
  let arr ← j.getArr?
  arr.toList.mapM parseTerm

/-! ## Main oracle logic -/

private def groundRulesJson (rules : List Rule) (domain : List Term) (idx : Nat := 0) : List Json :=
  match rules with
  | [] => []
  | rule :: rest =>
    let instances := rule.groundInstances domain
    let entry := Json.mkObj [
      ("rule_index", jsonNat idx),
      ("ground_instances", Json.arr (instances.map ruleToJson).toArray)
    ]
    entry :: groundRulesJson rest domain (idx + 1)

/-- Process a single grounding test case. -/
def processCase (j : Json) : Except String Json := do
  let rulesJ ← j.getObjValAs? (Array Json) "rules"
  let domainJ ← j.getObjVal? "domain"
  let rules ← rulesJ.toList.mapM parseRule
  let domain ← parseDomain domainJ
  let results := groundRulesJson rules domain
  .ok (Json.mkObj [("results", Json.arr results.toArray)])

/-- Process a batch of test cases. -/
def processBatch (j : Json) : Except String Json := do
  let cases ← j.getObjValAs? (Array Json) "cases"
  let results ← cases.toList.mapM processCase
  .ok (Json.mkObj [("results", Json.arr results.toArray)])

/-- Main entry point: read JSON from stdin, compute ground instances, write to stdout. -/
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

end Spindle.DiffTest.GroundingOracle

/-- Entry point for the GroundingOracle executable. -/
def main (args : List String) : IO UInt32 :=
  Spindle.DiffTest.GroundingOracle.main args
