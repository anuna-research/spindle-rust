/-
  SpindleLean.DiffTest.FamilyOracle
  Executable oracle for the temporal-family model (SpindleLean.Family).

  Protocol: JSONL batch. One theory per input line:

    {"rules":[{"label":"r0","type":"defeasible",
               "body":[{"name":"p","negated":false,"window":[1,10]}],
               "head":{"name":"q","negated":false}}],
     "superiority":[["r0","r1"]]}

  ("window" omitted or null = atemporal.)

  One response per line:

    {"conclusions":[{"literal":{"name":"p","negated":false,"window":[1,10]},
                     "type":"+D"}, ...]}
-/
import Lean.Data.Json
import SpindleLean.FamilyTwoSided

namespace Family.Oracle

open Lean (Json)

def parseWindow (j : Json) : Except String (Option Window) :=
  match j.getObjVal? "window" with
  | .error _ => pure none
  | .ok .null => pure none
  | .ok v => do
    let arr ← v.getArr?
    if h : arr.size = 2 then
      let s ← arr[0].getInt?
      let e ← arr[1].getInt?
      pure (some ⟨s, e⟩)
    else
      throw "window must be [start, stop]"

def parseMode (j : Json) : Except String (Option String) :=
  match j.getObjVal? "mode" with
  | .error _ => pure none
  | .ok .null => pure none
  | .ok v => do
    let s ← v.getStr?
    pure (some s)

def parseFLit (j : Json) : Except String FLit := do
  let name ← j.getObjVal? "name" >>= Json.getStr?
  let negated ← j.getObjVal? "negated" >>= Json.getBool?
  let mode ← parseMode j
  let window ← parseWindow j
  pure ⟨name, negated, mode, window⟩

def parseRuleType : String → Except String FRuleType
  | "fact" => pure .fact
  | "strict" => pure .strict
  | "defeasible" => pure .defeasible
  | "defeater" => pure .defeater
  | other => throw s!"unknown rule type {other}"

def parseFRule (j : Json) : Except String FRule := do
  let label ← j.getObjVal? "label" >>= Json.getStr?
  let tstr ← j.getObjVal? "type" >>= Json.getStr?
  let rtype ← parseRuleType tstr
  let bodyJ ← j.getObjVal? "body" >>= Json.getArr?
  let body ← bodyJ.toList.mapM parseFLit
  let head ← j.getObjVal? "head" >>= parseFLit
  pure ⟨label, rtype, body, head⟩

def parseFTheory (j : Json) : Except String FTheory := do
  let rulesJ ← j.getObjVal? "rules" >>= Json.getArr?
  let rules ← rulesJ.toList.mapM parseFRule
  let supJ ← j.getObjVal? "superiority" >>= Json.getArr?
  let sup ← supJ.toList.mapM fun p => do
    let arr ← p.getArr?
    if h : arr.size = 2 then do
      let w ← arr[0].getStr?
      let l ← arr[1].getStr?
      pure (w, l)
    else
      throw "superiority pair must be [winner, loser]"
  pure ⟨rules, sup⟩

def flitJson (l : FLit) : String :=
  let w := match l.window with
    | none => "null"
    | some iv => s!"[{iv.start},{iv.stop}]"
  let m := match l.mode with
    | none => "null"
    | some md => s!"\"{md}\""
  s!"\{\"name\":\"{l.name}\",\"negated\":{l.negated},\"mode\":{m},\"window\":{w}}"

def conclusionJson (l : FLit) (tag : String) : String :=
  s!"\{\"literal\":{flitJson l},\"type\":\"{tag}\"}"

/-- Run the two-sided family model (constructive defeat discard;
    FamilyTwoSided.lean) and produce all four conclusion tags over the
    theory's exact-literal universe. Final -d = universe \ proven,
    mirroring the engine's Phase-3 sweep. -/
def runTheory (t : FTheory) : String :=
  let (delta, partial_) := famReason2 t
  let entries := t.allLiterals.flatMap fun lit =>
    [conclusionJson lit (if delta.contains lit then "+D" else "-D"),
     conclusionJson lit (if partial_.contains lit then "+d" else "-d")]
  s!"\{\"conclusions\":[{String.intercalate "," entries}]}"

def processLine (line : String) : String :=
  match (Json.parse line >>= fun j =>
    (parseFTheory j).map runTheory : Except String String) with
  | .ok out => out
  | .error e => s!"\{\"error\":\"{e}\"}"

end Family.Oracle
