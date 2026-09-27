import Spindle.Aggregation.Operational

/-! The single-head SDL strong-permission profile (issue #44).
Atoms are opaque ground identities; optional windows model exact temporal
opposition and atemporal family premises. No parser or grounding claim is made.
-/
namespace Spindle.Modal

inductive Modality where
  | plain | obligation | permission | forbidden
  deriving DecidableEq, Repr, BEq

structure Lit where
  atom : String
  mode : Modality := .plain
  inner : Bool := false
  outer : Bool := false
  window : Option (Int × Int) := none
  deriving DecidableEq, Repr

instance : BEq Lit := ⟨fun a b => decide (a = b)⟩
instance : LawfulBEq Lit where
  eq_of_beq := of_decide_eq_true
  rfl := by intro a; change decide (a = a) = true; simp

def Lit.normalize (l : Lit) : Lit :=
  match l.mode with
  | .forbidden => { l with mode := .obligation, inner := !l.inner }
  | _ => l

def Lit.negate (l : Lit) : Lit :=
  if l.mode = .plain then { l with inner := !l.inner }
  else { l with outer := !l.outer }

def Lit.negativeModal (l : Lit) : Bool := decide (l.mode ≠ .plain) && l.outer

/-- Work on canonical literals. F is eliminated before this relation is used. -/
def conflict (a b : Lit) : Bool :=
  decide (a.atom = b.atom ∧ a.window = b.window) &&
    if a.mode = .plain then
      decide (b.mode = .plain ∧ a.outer = b.outer ∧ a.inner ≠ b.inner)
    else if a.outer || b.outer then
      decide (a.mode = b.mode ∧ a.inner = b.inner ∧ a.outer ≠ b.outer)
    else
      decide ((a.mode = .obligation ∨ a.mode = .permission) ∧
        (b.mode = .obligation ∨ b.mode = .permission) ∧
        a.inner ≠ b.inner ∧ (a.mode = .obligation ∨ b.mode = .obligation))

def opposes (a b : Lit) : Bool := conflict a.normalize b.normalize

def premiseMatches (premise candidate : Lit) : Bool :=
  decide (premise.atom = candidate.atom ∧ premise.mode = candidate.mode ∧
    premise.inner = candidate.inner ∧ premise.outer = candidate.outer) &&
    (premise.window.isNone || decide (premise.window = candidate.window))

structure MRule where
  label : String
  kind : RuleType
  body : List Lit
  head : Lit
  deriving DecidableEq, Repr

def MRule.normalize (r : MRule) : MRule :=
  { r with body := r.body.map Lit.normalize, head := r.head.normalize }

def MRule.productive (r : MRule) : Bool := decide (r.kind ≠ .defeater)

structure MTheory where
  rules : List MRule
  priority : List (String × String) := []
  deriving Repr

def MTheory.normalize (t : MTheory) : MTheory :=
  { t with rules := t.rules.map MRule.normalize }

/-- Superiority is explicitly declared, not transitively closed. -/
def MTheory.superior (t : MTheory) (a b : String) : Bool :=
  t.priority.contains (a, b)

/-- Team defense respects both the modal head and the attacker's rule class. -/
def defends (goal : Lit) (attacker defender : MRule) : Bool :=
  let same := decide (defender.head = goal)
  let cross := decide (goal.mode ≠ .plain ∧ goal.outer = false ∧
    defender.head.outer = false ∧
    (defender.head.mode = .obligation ∨ defender.head.mode = .permission) ∧
    defender.head.atom = goal.atom ∧ defender.head.inner = goal.inner ∧
    defender.head.window = goal.window) && conflict defender.head attacker.head
  (same || cross) &&
    if goal.mode = .plain ∨ goal.outer = true then defender.productive
    else if attacker.head.mode = .obligation ∧ attacker.head.outer = false ∧
        attacker.kind ≠ .defeater then true
    else defender.productive &&
      (attacker.head.outer || decide (defender.head.mode = .obligation))

end Spindle.Modal
