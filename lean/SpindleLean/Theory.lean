/-
  SpindleLean.Theory
  Theory structure: a collection of rules with a superiority relation.

  A theory T = (R, >) where R is a set of rules and > is a strict
  partial order (superiority relation) over rule labels.
-/
import SpindleLean.Rule
import Mathlib.Data.List.Dedup

/-- A defeasible logic theory -/
structure Theory where
  rules : List Rule
  /-- Superiority pairs: (winner, loser) means winner > loser -/
  superiority : List (String × String)
  deriving Repr

namespace Theory

/-- Empty theory -/
def empty : Theory := ⟨[], []⟩

/-- Add a rule to the theory -/
def addRule (t : Theory) (r : Rule) : Theory :=
  { t with rules := t.rules ++ [r] }

/-- Add a fact to the theory -/
def addFact (t : Theory) (label name : String) : Theory :=
  t.addRule (Rule.fact label (Literal.pos name))

/-- Add a strict rule -/
def addStrict (t : Theory) (label : String) (bodyNames : List String) (headName : String) : Theory :=
  t.addRule (Rule.strict label (bodyNames.map Literal.pos) (Literal.pos headName))

/-- Add a defeasible rule -/
def addDefeasible (t : Theory) (label : String) (bodyNames : List String) (headName : String) : Theory :=
  t.addRule (Rule.defeasible label (bodyNames.map Literal.pos) (Literal.pos headName))

/-- Declare r1 > r2 (r1 is superior to r2) -/
def addSuperiority (t : Theory) (winner loser : String) : Theory :=
  { t with superiority := t.superiority ++ [(winner, loser)] }

/-- Check if r1 > r2 -/
def isSuperior (t : Theory) (winner loser : String) : Bool :=
  t.superiority.any (fun (w, l) => w == winner && l == loser)

/-- Get all rules with a given literal as head -/
def rulesWithHead (t : Theory) (lit : Literal) : List Rule :=
  t.rules.filter (fun r => r.head == lit)

/-- Get all rules that contain a given literal in their body -/
def rulesWithBody (t : Theory) (lit : Literal) : List Rule :=
  t.rules.filter (fun r => r.body.contains lit)

/-- Get all facts in the theory -/
def facts (t : Theory) : List Rule :=
  t.rules.filter (fun r => r.ruleType == .fact)

/-- Get all distinct literals mentioned in the theory -/
def allLiterals (t : Theory) : List Literal :=
  let heads := t.rules.map (fun r => r.head)
  let bodies := t.rules.flatMap (fun r => r.body)
  (heads ++ bodies).dedup

/-- A theory is well-formed if every fact rule has an empty body -/
def WellFormed (t : Theory) : Prop :=
  ∀ r ∈ t.rules, r.ruleType = .fact → r.body = []

end Theory

instance : Inhabited Theory := ⟨Theory.empty⟩
