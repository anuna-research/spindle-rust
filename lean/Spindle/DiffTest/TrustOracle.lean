/-
  Spindle Trust Oracle

  Executable oracle for differential testing of the trust layer against
  `crates/spindle-core/src/trust.rs`. The Lean side computes over exact
  rationals; the Rust side computes in f64 and compares within a small
  tolerance.

  Protocol: JSONL batch (one request per input line, one response per
  output line, in order). Numbers in requests are JSON decimals (parsed
  exactly into ℚ). Responses carry exact rationals as {"num": n, "den": d},
  or {"bool": b} for threshold checks.

  Requests:
    {"op":"diminish",        "c":0.75, "d":0.5}
    {"op":"diminish_all",    "c":0.75, "ds":[0.5, 0.25]}
    {"op":"weakest_link",    "tree":{"trust":0.9,"children":[...]}}
    {"op":"linear_decay",    "rate":0.001, "age":100}
    {"op":"step_decay",      "cutoff":86400, "age":100}
    {"op":"effective_linear","base":0.8, "rate":0.001, "age":100}
    {"op":"effective_step",  "base":0.8, "cutoff":86400, "age":100}
    {"op":"threshold",       "v":0.7, "t":0.5}

  Run with: TrustOracle (reads stdin to EOF).
-/
import Lean.Data.Json
import Spindle.Trust.Diminish
import Spindle.Trust.WeakestLink
import Spindle.Trust.Decay

namespace Spindle.Trust.Oracle

open Lean (Json JsonNumber)

/-- Convert a JSON decimal number to an exact rational:
    mantissa * 10^(-exponent). -/
def jsonNumToRat (n : JsonNumber) : ℚ :=
  (n.mantissa : ℚ) / ((10 : ℚ) ^ n.exponent)

/-- Extract a rational field from a JSON object. -/
def getRat (j : Json) (field : String) : Except String ℚ := do
  match j.getObjVal? field with
  | .ok v =>
    match v.getNum? with
    | .ok n => pure (jsonNumToRat n)
    | .error e => throw s!"field {field}: {e}"
  | .error e => throw s!"missing field {field}: {e}"

/-- Extract an array of rationals. -/
def getRatArray (j : Json) (field : String) : Except String (List ℚ) := do
  match j.getObjVal? field with
  | .ok v =>
    match v.getArr? with
    | .ok arr =>
      arr.toList.mapM fun x =>
        match x.getNum? with
        | .ok n => pure (jsonNumToRat n)
        | .error e => throw s!"array element: {e}"
    | .error e => throw s!"field {field} not array: {e}"
  | .error e => throw s!"missing field {field}: {e}"

/-- Parse a derivation tree from JSON. Fuel-bounded recursion over the
    JSON structure. -/
partial def parseTree (j : Json) : Except String DerivationTree := do
  let trust ← getRat j "trust"
  let children ←
    match j.getObjVal? "children" with
    | .ok v =>
      match v.getArr? with
      | .ok arr => arr.toList.mapM parseTree
      | .error e => throw s!"children not array: {e}"
    | .error _ => pure []  -- children optional
  pure (.node trust children)

/-- Serialize a rational result. -/
def ratResponse (q : ℚ) : String :=
  s!"\{\"num\":{q.num},\"den\":{q.den}}"

/-- Serialize a boolean result. -/
def boolResponse (b : Bool) : String :=
  s!"\{\"bool\":{b}}"

/-- Process one request line. -/
def processLine (line : String) : String :=
  match (do
    let j ← Json.parse line
    let op ← (j.getObjVal? "op" >>= Json.getStr?)
    match op with
    | "diminish" => do
      let c ← getRat j "c"; let d ← getRat j "d"
      pure (ratResponse (diminish c d))
    | "diminish_all" => do
      let c ← getRat j "c"; let ds ← getRatArray j "ds"
      pure (ratResponse (diminishAll c ds))
    | "weakest_link" => do
      let tj ← j.getObjVal? "tree"
      let tree ← parseTree tj
      pure (ratResponse tree.weakestLink)
    | "linear_decay" => do
      let rate ← getRat j "rate"; let age ← getRat j "age"
      pure (ratResponse (linearDecay rate age))
    | "step_decay" => do
      let cutoff ← getRat j "cutoff"; let age ← getRat j "age"
      pure (ratResponse (stepDecay cutoff age))
    | "effective_linear" => do
      let base ← getRat j "base"; let rate ← getRat j "rate"; let age ← getRat j "age"
      pure (ratResponse (effectiveTrust base (linearDecay rate age)))
    | "effective_step" => do
      let base ← getRat j "base"; let cutoff ← getRat j "cutoff"; let age ← getRat j "age"
      pure (ratResponse (effectiveTrust base (stepDecay cutoff age)))
    | "threshold" => do
      let v ← getRat j "v"; let t ← getRat j "t"
      pure (boolResponse (t ≤ v))
    | other => throw s!"unknown op: {other}"
    : Except String String) with
  | .ok out => out
  | .error e => s!"\{\"error\":\"{e}\"}"

end Spindle.Trust.Oracle

def main (_ : List String) : IO Unit := do
  let stdin ← IO.getStdin
  let stdout ← IO.getStdout
  let input ← stdin.readToEnd
  for line in input.splitOn "\n" do
    let line := line.trimAscii.toString
    if !line.isEmpty then
      stdout.putStrLn (Spindle.Trust.Oracle.processLine line)
  stdout.flush
