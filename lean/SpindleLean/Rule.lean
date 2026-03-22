/-
  SpindleLean.Rule
  Rule types and structure for defeasible logic theories.

  Four rule types form the DL hierarchy:
  - Fact:       >> p         (unconditional truth)
  - Strict:     body -> p    (monotonic, cannot be defeated)
  - Defeasible: body => p    (can be defeated by superior rules)
  - Defeater:   body ~> p    (blocks conclusions, never proves)
-/
import SpindleLean.Basic

/-- The four rule types in defeasible logic -/
inductive RuleType where
  | fact        -- >>  unconditional
  | strict      -- ->  cannot be defeated
  | defeasible  -- =>  can be defeated
  | defeater    -- ~>  blocks only
  deriving DecidableEq, Repr, BEq, Hashable

/-- A rule in a defeasible logic theory -/
structure Rule where
  label : String
  ruleType : RuleType
  body : List Literal
  head : Literal
  deriving DecidableEq, Repr, BEq

namespace Rule

/-- Create a fact rule -/
def fact (label : String) (head : Literal) : Rule :=
  ⟨label, .fact, [], head⟩

/-- Create a strict rule -/
def strict (label : String) (body : List Literal) (head : Literal) : Rule :=
  ⟨label, .strict, body, head⟩

/-- Create a defeasible rule -/
def defeasible (label : String) (body : List Literal) (head : Literal) : Rule :=
  ⟨label, .defeasible, body, head⟩

/-- Create a defeater rule -/
def defeater (label : String) (body : List Literal) (head : Literal) : Rule :=
  ⟨label, .defeater, body, head⟩

/-- Whether a rule is a fact -/
def isFact (r : Rule) : Bool := r.ruleType == .fact

/-- Whether a rule can derive conclusions (not a defeater) -/
def isProductive (r : Rule) : Bool :=
  r.ruleType == .fact || r.ruleType == .strict || r.ruleType == .defeasible

/-- Whether a rule participates in definite provability -/
def isDefinite (r : Rule) : Bool :=
  r.ruleType == .fact || r.ruleType == .strict

/-- Whether the rule body is satisfied by a set of literals -/
def bodySatisfied (r : Rule) (proven : List Literal) : Bool :=
  r.body.all (proven.contains ·)

end Rule
