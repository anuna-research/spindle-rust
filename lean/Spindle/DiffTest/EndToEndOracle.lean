/-
  Spindle End-to-End Oracle

  JSON-based oracle for differential testing of the full pipeline:
  parse → ground → reason → conclude.

  Takes a non-ground theory (rules with variables) and a domain,
  grounds all rules via Herbrand enumeration, then runs semi-naive
  forward chaining on the ground rules, and returns the derived facts.

  JSON wire format:
  - Input:
    {
      "rules": [{"head": <Literal>, "body": [<Literal>]}],
      "domain": [<Term>],
      "fuel": N  (optional, default 100)
    }
  - Output:
    {
      "ground_rules": [{"head": <Literal>, "body": [<Literal>]}],
      "derived_facts": [<Literal>]
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
import Spindle.Arith.SemiNaive
import Lean.Data.Json

open Lean (Json JsonNumber)

namespace Spindle.DiffTest.EndToEndOracle

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
  | .variable v => Json.mkObj [("variable", Json.str v)]

private def parseTerm (j : Json) : Except String Term := do
  if let some s := j.getObjValAs? String "symbol" |>.toOption then
    return .symbol s
  if let some n := j.getObjValAs? Int "integer" |>.toOption then
    return .integer n
  if let some obj := j.getObjVal? "decimal" |>.toOption then
    let n ← obj.getObjValAs? Int "n"
    let scale ← obj.getObjValAs? Nat "scale"
    return .decimal n scale
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

/-! ## Deduplication helper -/

/-- Remove duplicate literals (BEq-based). -/
private def dedupLiterals (ls : List Literal) : List Literal :=
  ls.foldl (fun acc l => if acc.elem l then acc else acc ++ [l]) []

/-! ## Main oracle logic -/

/-- Process a single end-to-end test case:
    1. Ground all rules over the domain
    2. Run semi-naive evaluation on the ground rules
    3. Return both ground rules and derived facts -/
def processCase (j : Json) : Except String Json := do
  let rulesJ ← j.getObjValAs? (Array Json) "rules"
  let domainJ ← j.getObjVal? "domain"
  let rules ← rulesJ.toList.mapM parseRule
  let domain ← parseDomain domainJ
  let fuel := (j.getObjValAs? Nat "fuel" |>.toOption).getD 100

  -- Step 1: Ground all rules
  let groundRules := rules.flatMap (fun r => r.groundInstances domain)

  -- Step 2: Run semi-naive evaluation
  let derivedFacts := semiNaiveEval groundRules fuel

  -- Step 3: Deduplicate derived facts
  let uniqueFacts := dedupLiterals derivedFacts

  .ok (Json.mkObj [
    ("ground_rules", Json.arr (groundRules.map ruleToJson).toArray),
    ("derived_facts", Json.arr (uniqueFacts.map literalToJson).toArray)
  ])

/-- Process a batch of test cases. -/
def processBatch (j : Json) : Except String Json := do
  let cases ← j.getObjValAs? (Array Json) "cases"
  let results ← cases.toList.mapM processCase
  .ok (Json.mkObj [("results", Json.arr results.toArray)])

/-- Main entry point: read JSON from stdin, run end-to-end pipeline, write to stdout. -/
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

end Spindle.DiffTest.EndToEndOracle

/-- Entry point for the EndToEndOracle executable. -/
def main (args : List String) : IO UInt32 :=
  Spindle.DiffTest.EndToEndOracle.main args
