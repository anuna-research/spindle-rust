import SpindleLean.Reason
import Lean.Data.Json

/-!
# SpindleLean.DiffTest.Oracle

Differential testing oracle: JSON serialization/parsing for cross-validation
against the Rust implementation.

## JSON Format

### Input (Theory)
```json
{
  "rules": [
    {
      "label": "r1",
      "rule_type": "strict",
      "body": [{"name": "a", "polarity": true}],
      "head": {"name": "b", "polarity": true}
    }
  ],
  "superiority": [["r1", "r2"]]
}
```

### Output (Conclusions)
```json
{
  "conclusions": [
    {"conclusion_type": "+D", "literal": {"name": "a", "polarity": true}}
  ]
}
```
-/

open Lean (Json ToJson FromJson toJson fromJson?)

namespace SpindleLean.DiffTest

-- ============================================================================
-- FromJson instances (parsing JSON → Lean types)
-- ============================================================================

instance : FromJson Literal where
  fromJson? j := do
    let name ← j.getObjValAs? String "name"
    let polarity ← j.getObjValAs? Bool "polarity"
    return { name, polarity }

instance : FromJson RuleType where
  fromJson? j := do
    let s ← j.getStr?
    match s with
    | "strict"     => return .strict
    | "defeasible" => return .defeasible
    | "defeater"   => return .defeater
    | other        => throw s!"unknown rule_type: {other}"

instance : FromJson Rule where
  fromJson? j := do
    let label ← j.getObjValAs? String "label"
    let ruleType ← j.getObjValAs? RuleType "rule_type"
    let body ← j.getObjValAs? (List Literal) "body"
    let head ← j.getObjValAs? Literal "head"
    return { label, ruleType, body, head }

instance : FromJson Theory where
  fromJson? j := do
    let rules ← j.getObjValAs? (List Rule) "rules"
    let supArr ← j.getObjValAs? (List (List String)) "superiority"
    let superiority ← supArr.mapM fun pair =>
      match pair with
      | [a, b] => pure (a, b)
      | _ => throw "superiority pairs must be arrays of exactly 2 strings"
    return { rules, superiority }

-- ============================================================================
-- ToJson instances (Lean types → JSON output)
-- ============================================================================

instance : ToJson Literal where
  toJson l := Json.mkObj [
    ("name", toJson l.name),
    ("polarity", toJson l.polarity)
  ]

instance : ToJson ConclusionType where
  toJson ct := toJson ct.symbol

instance : ToJson Conclusion where
  toJson c := Json.mkObj [
    ("conclusion_type", toJson c.conclusionType),
    ("literal", toJson c.literal)
  ]

-- ============================================================================
-- Oracle entry point
-- ============================================================================

/-- Parse a JSON string into a Theory. -/
def parseTheory (input : String) : Except String Theory := do
  let json ← Json.parse input
  match fromJson? json with
  | .ok t => .ok t
  | .error e => .error s!"failed to parse theory: {e}"

/-- Run the oracle: parse theory from JSON, compute conclusions, return JSON. -/
def runOracle (input : String) : Except String String := do
  let theory ← parseTheory input
  let conclusions := reason theory
  let output := Json.mkObj [
    ("conclusions", toJson conclusions)
  ]
  return output.compress

/-- IO action: read JSON theory from stdin, write JSON conclusions to stdout. -/
def oracleMain : IO Unit := do
  let stdin ← IO.getStdin
  let input ← stdin.readToEnd
  match runOracle input with
  | .ok output => IO.println output
  | .error e =>
    IO.eprintln s!"error: {e}"
    IO.Process.exit 1

end SpindleLean.DiffTest
