import Lean.Data.Json
import Spindle.Modal.Properties

/-! JSONL test adapter for finite ground modal theories. This adapter is not
a verified SPL parser. Each response reports all four tags for each query,
in the order +D, -D, +d, -d. Malformed requests produce an explicit error. -/
namespace Spindle.Modal.Oracle
open Lean

def parseLit (j : Json) : Except String Lit := do
  let atom ← j.getObjValAs? String "atom"
  let mode ← match ← j.getObjValAs? String "mode" with
    | "plain" => pure Modality.plain
    | "O" => pure Modality.obligation
    | "P" => pure Modality.permission
    | "F" => pure Modality.forbidden
    | other => throw s!"unknown modality: {other}"
  let inner ← j.getObjValAs? Bool "inner"
  let outer ← j.getObjValAs? Bool "outer"
  if mode = .plain && outer then throw "plain literals have no outer modality"
  let window ← match ← j.getObjVal? "window" with
    | .null => pure none
    | value => do
      let pair ← value.getArr?
      if pair.size != 2 then throw "window must contain two endpoints"
      let start ← pair[0]!.getInt?
      let stop ← pair[1]!.getInt?
      if start > stop then throw "window endpoints out of order"
      pure (some (start, stop))
  pure ⟨atom, mode, inner, outer, window⟩

def parseRule (j : Json) : Except String MRule := do
  let label ← j.getObjValAs? String "label"
  let kind ← match ← j.getObjValAs? String "kind" with
    | "fact" => pure RuleType.fact
    | "strict" => pure RuleType.strict
    | "defeasible" => pure RuleType.defeasible
    | "defeater" => pure RuleType.defeater
    | other => throw s!"unknown rule kind: {other}"
  let body ← (← (← j.getObjVal? "body").getArr?).toList.mapM parseLit
  if kind = .fact && !body.isEmpty then throw "facts must have empty bodies"
  let head ← parseLit (← j.getObjVal? "head")
  pure ⟨label, kind, body, head⟩

def evaluate (j : Json) : Except String Json := do
  let rules ← (← (← j.getObjVal? "rules").getArr?).toList.mapM parseRule
  let labels := rules.map MRule.label
  if labels.dedup.length != labels.length then throw "rule labels must be unique"
  let priority ← (← (← j.getObjVal? "priority").getArr?).toList.mapM fun p => do
    let pair ← p.getArr?
    if pair.size != 2 then throw "priority must contain two labels"
    pure (← pair[0]!.getStr?, ← pair[1]!.getStr?)
  let queries ← (← (← j.getObjVal? "queries").getArr?).toList.mapM parseLit
  let state := reason ⟨rules, priority⟩
  pure <| Json.mkObj [("tags", toJson (queries.map fun l =>
    tags.map fun tag => has state tag l.normalize))]

def process (line : String) : String :=
  match Json.parse line >>= evaluate with
  | .ok result => result.compress
  | .error error => (Json.mkObj [("error", toJson error)]).compress

end Spindle.Modal.Oracle

def main : IO Unit := do
  let stdin ← IO.getStdin
  let stdout ← IO.getStdout
  for line in (← stdin.readToEnd).splitOn "\n" do
    if !line.trimAscii.toString.isEmpty then
      stdout.putStrLn (Spindle.Modal.Oracle.process line)
  stdout.flush
