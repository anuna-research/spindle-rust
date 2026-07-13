/-
  SpindleLean.Reason
  Top-level reasoning: compute all three closures and conclusions.

  Follows the DL(d||) three-phase pattern from scalable.rs:
    Delta  = definite conclusions (+D)
    Lambda = over-approximation
    Partial = defeasible conclusions (+d)
-/
import SpindleLean.Closure.Partial

/-- Conclusion types in defeasible logic -/
inductive ConclusionType where
  | definitelyProvable    -- +D
  | definitelyNotProvable -- -D
  | defeasiblyProvable    -- +d
  | defeasiblyNotProvable -- -d
  deriving DecidableEq, Repr, BEq

/-- A conclusion: a literal tagged with its provability status -/
structure Conclusion where
  literal : Literal
  conclusionType : ConclusionType
  deriving Repr

/-- Result of reasoning over a theory -/
structure ReasonResult where
  delta : List Literal
  lambda : List Literal
  partial_ : List Literal
  conclusions : List Conclusion
  deriving Repr

namespace ReasonResult

def containsDelta (r : ReasonResult) (name : String) (negated : Bool := false) : Bool :=
  r.delta.contains ⟨name, negated, none⟩

def containsPartial (r : ReasonResult) (name : String) (negated : Bool := false) : Bool :=
  r.partial_.contains ⟨name, negated, none⟩

def containsLambda (r : ReasonResult) (name : String) (negated : Bool := false) : Bool :=
  r.lambda.contains ⟨name, negated, none⟩

end ReasonResult

/-- Run the three-phase DL(d||) reasoning algorithm -/
def reason (t : Theory) : ReasonResult :=
  let delta := Closure.deltaClose t
  let lambda := Closure.lambdaClose t delta
  let partial_ := Closure.partialClose t delta lambda
  let allLits := t.allLiterals
  let conclusions := allLits.flatMap fun lit => [
    if delta.contains lit then
      { literal := lit, conclusionType := .definitelyProvable }
    else
      { literal := lit, conclusionType := .definitelyNotProvable },
    if partial_.contains lit then
      { literal := lit, conclusionType := .defeasiblyProvable }
    else
      { literal := lit, conclusionType := .defeasiblyNotProvable }
  ]
  { delta, lambda, partial_, conclusions }
