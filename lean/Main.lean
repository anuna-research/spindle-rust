import SpindleLean

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

def main (args : List String) : IO Unit := do
  match args with
  | ["--oracle"] => runOracle
  | _ => runTweetyTest
