/-
  SpindleLean.Closure.Delta
  Delta closure: definite provability via facts and strict rules only.

  Delta(T) is the least fixed point of the delta operator, which
  collects all literals derivable through facts and strict rules.
  Corresponds to +D conclusions in DL(d).
-/
import SpindleLean.Theory
import Mathlib.Data.List.Dedup

namespace Closure

/-- One step of delta closure: add heads of fact/strict rules whose bodies
    are all in the current set -/
def deltaStep (t : Theory) (current : List Literal) : List Literal :=
  let newLits := t.rules.filterMap fun r =>
    if r.isDefinite && r.bodySatisfied current && !current.contains r.head then
      some r.head
    else
      none
  (current ++ newLits).dedup

/-- Compute delta closure by iterating to fixpoint with bounded fuel -/
def deltaClose (t : Theory) (fuel : Nat := 1000) : List Literal :=
  go t (t.facts.map (·.head)).dedup fuel
where
  go (t : Theory) (current : List Literal) : Nat → List Literal
    | 0 => current
    | fuel + 1 =>
      let next := deltaStep t current
      if next.length == current.length then current
      else go t next fuel

/-- Check if a literal is definitely provable -/
def inDelta (t : Theory) (name : String) (negated : Bool := false) : Bool :=
  let delta := deltaClose t
  delta.contains ⟨name, negated, none⟩

end Closure
