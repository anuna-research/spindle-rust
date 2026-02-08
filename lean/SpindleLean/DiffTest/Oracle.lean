/-
  SpindleLean.DiffTest.Oracle
  Executable oracle for differential testing against Rust.

  Reads a JSON theory from stdin, runs reasoning, and outputs
  JSON conclusions to stdout. Format:

  Input:
  {
    "rules": [
      {"label": "f1", "type": "fact", "body": [], "head": {"name": "p", "negated": false}},
      {"label": "r1", "type": "defeasible", "body": [{"name": "p", "negated": false}],
       "head": {"name": "q", "negated": false}}
    ],
    "superiority": [["r2", "r1"]]
  }

  Output:
  {
    "delta": [{"name": "p", "negated": false}],
    "lambda": [...],
    "partial": [...],
    "conclusions": [
      {"literal": {"name": "p", "negated": false}, "type": "+D"},
      {"literal": {"name": "p", "negated": false}, "type": "+d"}
    ]
  }
-/
import SpindleLean.Reason

set_option linter.deprecated false

namespace DiffTest

-- ═══════════════════════════════════════════════════════════════
-- JSON serialization (simple string-based, no external deps)
-- ═══════════════════════════════════════════════════════════════

/-- Serialize a Literal to JSON string -/
def literalToJson (l : Literal) : String :=
  let modeStr := match l.mode with
    | none => "null"
    | some .obligation => "\"obligation\""
    | some .permission => "\"permission\""
    | some .forbidden => "\"forbidden\""
  "{\"name\":\"" ++ l.name ++ "\",\"negated\":" ++ toString l.negated ++ ",\"mode\":" ++ modeStr ++ "}"

/-- Serialize a list of Literals to JSON array -/
def literalListToJson (ls : List Literal) : String :=
  "[" ++ ",".intercalate (ls.map literalToJson) ++ "]"

/-- Serialize a ConclusionType to its DL notation -/
def conclusionTypeToStr : ConclusionType → String
  | .definitelyProvable => "+D"
  | .definitelyNotProvable => "-D"
  | .defeasiblyProvable => "+d"
  | .defeasiblyNotProvable => "-d"

/-- Serialize a Conclusion to JSON -/
def conclusionToJson (c : Conclusion) : String :=
  "{\"literal\":" ++ literalToJson c.literal ++ ",\"type\":\"" ++ conclusionTypeToStr c.conclusionType ++ "\"}"

/-- Serialize ReasonResult to JSON -/
def resultToJson (r : ReasonResult) : String :=
  let delta := literalListToJson r.delta
  let lambda := literalListToJson r.lambda
  let partial_ := literalListToJson r.partial_
  let conclusions := "[" ++ ",".intercalate (r.conclusions.map conclusionToJson) ++ "]"
  "{\"delta\":" ++ delta ++ ",\"lambda\":" ++ lambda ++ ",\"partial\":" ++ partial_ ++ ",\"conclusions\":" ++ conclusions ++ "}"

-- ═══════════════════════════════════════════════════════════════
-- Simple JSON parser (sufficient for our structured input)
-- ═══════════════════════════════════════════════════════════════

/-- Position type alias for readability -/
abbrev Pos := String.Pos.Raw

/-- Get byte index of end of string as Pos -/
def endPos (s : String) : Pos := s.endPos.offset

/-- Skip whitespace in a string starting at position i -/
partial def skipWs (s : String) (i : Pos) : Pos :=
  if i.byteIdx < (endPos s).byteIdx then
    let c := s.get i
    if c == ' ' || c == '\n' || c == '\r' || c == '\t' then
      skipWs s (s.next i)
    else i
  else i

/-- Extract a JSON string value (assumes pos is at opening quote) -/
partial def parseJsonString (s : String) (i : Pos) : Option (String × Pos) :=
  if s.get i != '"' then none
  else
    let start := s.next i
    let rec go (pos : Pos) (acc : String) : Option (String × Pos) :=
      if pos.byteIdx < (endPos s).byteIdx then
        let c := s.get pos
        if c == '"' then some (acc, s.next pos)
        else go (s.next pos) (acc.push c)
      else none
    go start ""

/-- Parse a boolean at position -/
def parseJsonBool (s : String) (i : Pos) : Option (Bool × Pos) :=
  if s.substrEq i "true" 0 4 then some (true, ⟨i.byteIdx + 4⟩)
  else if s.substrEq i "false" 0 5 then some (false, ⟨i.byteIdx + 5⟩)
  else none

/-- Expect a specific character, return new position -/
def expectChar (s : String) (i : Pos) (c : Char) : Option Pos :=
  let i := skipWs s i
  if i.byteIdx < (endPos s).byteIdx && s.get i == c then some (s.next i)
  else none

/-- Parse a Literal from JSON -/
def parseLiteral (s : String) (i : Pos) : Option (Literal × Pos) := do
  let i ← expectChar s i '{'
  -- expect "name":
  let i := skipWs s i
  let (key, i) ← parseJsonString s i
  if key != "name" then none
  let i ← expectChar s i ':'
  let i := skipWs s i
  let (name, i) ← parseJsonString s i
  let i ← expectChar s i ','
  -- expect "negated":
  let i := skipWs s i
  let (key2, i) ← parseJsonString s i
  if key2 != "negated" then none
  let i ← expectChar s i ':'
  let i := skipWs s i
  let (negated, i) ← parseJsonBool s i
  -- optional mode field or closing brace
  let i := skipWs s i
  if i.byteIdx < (endPos s).byteIdx && s.get i == '}' then
    return (⟨name, negated, none⟩, s.next i)
  else do
    let i ← expectChar s i ','
    let i := skipWs s i
    let (key3, i) ← parseJsonString s i
    if key3 != "mode" then none
    let i ← expectChar s i ':'
    let i := skipWs s i
    -- parse mode value (string or null)
    if s.substrEq i "null" 0 4 then
      let i : Pos := ⟨i.byteIdx + 4⟩
      let i ← expectChar s i '}'
      return (⟨name, negated, none⟩, i)
    else do
      let (modeStr, i) ← parseJsonString s i
      let mode := match modeStr with
        | "obligation" => some Mode.obligation
        | "permission" => some Mode.permission
        | "forbidden" => some Mode.forbidden
        | _ => none
      let i ← expectChar s i '}'
      return (⟨name, negated, mode⟩, i)

/-- Parse a JSON array of Literals -/
def parseLiteralArray (s : String) (i : Pos) : Option (List Literal × Pos) := do
  let i ← expectChar s i '['
  let i := skipWs s i
  if i.byteIdx < (endPos s).byteIdx && s.get i == ']' then
    return ([], s.next i)
  else do
    let rec go (pos : Pos) (acc : List Literal) (fuel : Nat) :
        Option (List Literal × Pos) := do
      match fuel with
      | 0 => none
      | fuel + 1 =>
        let pos := skipWs s pos
        let (lit, pos) ← parseLiteral s pos
        let pos := skipWs s pos
        if pos.byteIdx < (endPos s).byteIdx && s.get pos == ',' then
          go (s.next pos) (acc ++ [lit]) fuel
        else if pos.byteIdx < (endPos s).byteIdx && s.get pos == ']' then
          return (acc ++ [lit], s.next pos)
        else none
    go i [] 1000

/-- Parse a RuleType from string -/
def parseRuleType (s : String) : Option RuleType :=
  match s with
  | "fact" => some .fact
  | "strict" => some .strict
  | "defeasible" => some .defeasible
  | "defeater" => some .defeater
  | _ => none

/-- Parse a Rule from JSON -/
def parseRule (s : String) (i : Pos) : Option (Rule × Pos) := do
  let i ← expectChar s i '{'
  let i := skipWs s i
  -- parse fields in order: label, type, body, head
  let (key, i) ← parseJsonString s i
  if key != "label" then none
  let i ← expectChar s i ':'
  let i := skipWs s i
  let (label, i) ← parseJsonString s i
  let i ← expectChar s i ','
  let i := skipWs s i
  let (key2, i) ← parseJsonString s i
  if key2 != "type" then none
  let i ← expectChar s i ':'
  let i := skipWs s i
  let (typeStr, i) ← parseJsonString s i
  let ruleType ← parseRuleType typeStr
  let i ← expectChar s i ','
  let i := skipWs s i
  let (key3, i) ← parseJsonString s i
  if key3 != "body" then none
  let i ← expectChar s i ':'
  let i := skipWs s i
  let (body, i) ← parseLiteralArray s i
  let i ← expectChar s i ','
  let i := skipWs s i
  let (key4, i) ← parseJsonString s i
  if key4 != "head" then none
  let i ← expectChar s i ':'
  let i := skipWs s i
  let (head, i) ← parseLiteral s i
  let i ← expectChar s i '}'
  return (⟨label, ruleType, body, head⟩, i)

/-- Parse a superiority pair ["winner", "loser"] -/
def parseSupPair (s : String) (i : Pos) : Option ((String × String) × Pos) := do
  let i ← expectChar s i '['
  let i := skipWs s i
  let (winner, i) ← parseJsonString s i
  let i ← expectChar s i ','
  let i := skipWs s i
  let (loser, i) ← parseJsonString s i
  let i ← expectChar s i ']'
  return ((winner, loser), i)

/-- Parse a JSON array with a given element parser -/
def parseArray {α : Type} (parseElem : String → Pos → Option (α × Pos))
    (s : String) (i : Pos) : Option (List α × Pos) := do
  let i ← expectChar s i '['
  let i := skipWs s i
  if i.byteIdx < (endPos s).byteIdx && s.get i == ']' then
    return ([], s.next i)
  else do
    let rec go (pos : Pos) (acc : List α) (fuel : Nat) :
        Option (List α × Pos) := do
      match fuel with
      | 0 => none
      | fuel + 1 =>
        let pos := skipWs s pos
        let (elem, pos) ← parseElem s pos
        let pos := skipWs s pos
        if pos.byteIdx < (endPos s).byteIdx && s.get pos == ',' then
          go (s.next pos) (acc ++ [elem]) fuel
        else if pos.byteIdx < (endPos s).byteIdx && s.get pos == ']' then
          return (acc ++ [elem], s.next pos)
        else none
    go i [] 10000

/-- Parse a Theory from JSON -/
def parseTheory (s : String) : Option Theory := do
  let i := skipWs s ⟨0⟩
  let i ← expectChar s i '{'
  let i := skipWs s i
  -- "rules":
  let (key, i) ← parseJsonString s i
  if key != "rules" then none
  let i ← expectChar s i ':'
  let i := skipWs s i
  let (rules, i) ← parseArray parseRule s i
  let i ← expectChar s i ','
  let i := skipWs s i
  -- "superiority":
  let (key2, i) ← parseJsonString s i
  if key2 != "superiority" then none
  let i ← expectChar s i ':'
  let i := skipWs s i
  let (sup, _) ← parseArray parseSupPair s i
  return ⟨rules, sup⟩

-- ═══════════════════════════════════════════════════════════════
-- Oracle entry point
-- ═══════════════════════════════════════════════════════════════

/-- Run the oracle: parse theory from string, reason, return JSON -/
def runOracle (input : String) : String :=
  match parseTheory input with
  | none => "{\"error\":\"Failed to parse theory JSON\"}"
  | some theory =>
    let result := reason theory
    resultToJson result

end DiffTest
