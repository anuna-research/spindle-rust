/-
  SpindleLean.Closure.Lambda
  Lambda closure: over-approximation of defeasible provability.

  Lambda(T) includes everything that COULD be defeasibly provable
  (ignoring defeat). Key invariant: if p in Lambda and ~p in Delta,
  then p is permanently blocked.

  Uses facts + strict + defeasible rules, but excludes literals
  whose complement is in Delta (definitely proven opposite).
-/
import SpindleLean.Closure.Delta

namespace Closure

/-- One step of lambda closure: add heads of fact/strict/defeasible rules
    whose bodies are in current and whose complement is NOT in delta -/
def lambdaStep (t : Theory) (delta : List Literal) (current : List Literal) : List Literal :=
  let newLits := t.rules.filterMap fun r =>
    if r.isProductive && r.bodySatisfied current
       && !current.contains r.head
       && !delta.contains r.head.complement then
      some r.head
    else
      none
  (current ++ newLits).dedup

/-- Compute lambda closure by iterating to fixpoint. Default fuel is
    derived from the finite literal universe (see `deltaClose`). -/
def lambdaClose (t : Theory) (delta : List Literal)
    (fuel : Nat := t.allLiterals.length + 1) : List Literal :=
  go t delta delta fuel
where
  go (t : Theory) (delta current : List Literal) : Nat → List Literal
    | 0 => current
    | fuel + 1 =>
      let next := lambdaStep t delta current
      if next.length == current.length then current
      else go t delta next fuel

end Closure
