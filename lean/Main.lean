import SpindleLean
import Spindle.Spl.Grammar

/-
  Tweety Triangle: the canonical defeasible logic example.

  Facts: bird(tweety), penguin(tweety), bird(eddie)
  Rules:
    r1: bird(tweety) => flies(tweety)
    r2: penguin(tweety) => ~flies(tweety)
    r3: bird(eddie) => flies(eddie)
  Superiority: r2 > r1 (penguins override birds)

  Expected:
    +d flies(eddie)       -- eddie flies (no conflict)
    +d ~flies(tweety)     -- tweety doesn't fly (r2 beats r1)
    -d flies(tweety)      -- tweety flying is defeated
-/
def tweetyTheory : Theory :=
  Theory.empty
    |>.addRule (Rule.fact "f1" (Literal.pos "bird_tweety"))
    |>.addRule (Rule.fact "f2" (Literal.pos "penguin_tweety"))
    |>.addRule (Rule.fact "f3" (Literal.pos "bird_eddie"))
    |>.addRule (Rule.defeasible "r1" [Literal.pos "bird_tweety"] (Literal.pos "flies_tweety"))
    |>.addRule (Rule.defeasible "r2" [Literal.pos "penguin_tweety"] (Literal.neg "flies_tweety"))
    |>.addRule (Rule.defeasible "r3" [Literal.pos "bird_eddie"] (Literal.pos "flies_eddie"))
    |>.addSuperiority "r2" "r1"

def runTweetyTest : IO Unit := do
  let result := reason tweetyTheory

  IO.println "=== Tweety Triangle ==="
  IO.println ""

  IO.println s!"Delta (definite): {result.delta.map (fun l => if l.negated then s!"~{l.name}" else l.name)}"
  IO.println s!"Lambda (approx):  {result.lambda.map (fun l => if l.negated then s!"~{l.name}" else l.name)}"
  IO.println s!"Partial (defeas): {result.partial_.map (fun l => if l.negated then s!"~{l.name}" else l.name)}"
  IO.println ""

  -- Verify expected conclusions
  let checks := [
    ("flies_eddie is +d",    result.containsPartial "flies_eddie"),
    ("~flies_tweety is +d",  result.containsPartial "flies_tweety" true),
    ("flies_tweety is -d",   !result.containsPartial "flies_tweety"),
  ]

  for (desc, ok) in checks do
    IO.println s!"  {if ok then "PASS" else "FAIL"}: {desc}"

  if checks.all (·.2) then
    IO.println "\nAll checks passed!"
  else
    IO.println "\nSome checks FAILED!"
    IO.Process.exit 1

def runOracle : IO Unit := do
  let stdin ← IO.getStdin
  let input ← stdin.readToEnd
  let output := DiffTest.runOracle input
  IO.println output

/-- Batch mode: one theory JSON per input line (JSONL), one result JSON per
    output line, in order. Amortizes process startup over many small cases —
    used by the exhaustive small-scope difftest on the Rust side. -/
def runOracleBatch : IO Unit := do
  let stdin ← IO.getStdin
  let stdout ← IO.getStdout
  let input ← stdin.readToEnd
  for line in input.splitOn "\n" do
    let line := line.trimAscii.toString
    if !line.isEmpty then
      stdout.putStrLn (DiffTest.runOracle line)
  stdout.flush

/-- Batch mode for the temporal-family model: one theory JSON per line,
    one result per line (see SpindleLean/DiffTest/FamilyOracle.lean). -/
def runFamilyOracleBatch : IO Unit := do
  let stdin ← IO.getStdin
  let stdout ← IO.getStdout
  let input ← stdin.readToEnd
  for line in input.splitOn "\n" do
    let line := line.trimAscii.toString
    if !line.isEmpty then
      stdout.putStrLn (Family.Oracle.processLine line)
  stdout.flush

namespace SplParse

def litJson (l : Spl.SplLit) : String :=
  s!"\{\"name\":\"{l.name}\",\"negated\":{l.negated}}"

def typeStr : Spl.SplRuleType → String
  | .strict => "strict"
  | .defeasible => "defeasible"
  | .defeater => "defeater"

def stmtJson : Spl.SplStmt → String
  | .fact l => s!"\{\"kind\":\"fact\",\"head\":{litJson l}}"
  | .rule t label body head =>
    let bodyJ := String.intercalate "," (body.map litJson)
    s!"\{\"kind\":\"rule\",\"type\":\"{typeStr t}\",\"label\":\"{label}\",\"body\":[{bodyJ}],\"head\":{litJson head}}"
  | .prefer w l => s!"\{\"kind\":\"prefer\",\"winner\":\"{w}\",\"loser\":\"{l}\"}"

def processLine (line : String) : String :=
  match Spl.parseTheory line with
  | some t =>
    s!"\{\"stmts\":[{String.intercalate "," (t.map stmtJson)}]}"
  | none => "{\"error\":\"parse failed\"}"

end SplParse

/-- Batch mode for the SPL grammar fragment: one theory (whitespace-joined
    statements) per input line; one normalized-JSON parse result per output
    line. -/
def runSplParseBatch : IO Unit := do
  let stdin ← IO.getStdin
  let stdout ← IO.getStdout
  let input ← stdin.readToEnd
  for line in input.splitOn "\n" do
    let line := line.trimAscii.toString
    if !line.isEmpty then
      stdout.putStrLn (SplParse.processLine line)
  stdout.flush

def main (args : List String) : IO Unit := do
  match args with
  | ["--oracle"] => runOracle
  | ["--oracle-batch"] => runOracleBatch
  | ["--oracle-family-batch"] => runFamilyOracleBatch
  | ["--parse-spl-batch"] => runSplParseBatch
  | _ => runTweetyTest
