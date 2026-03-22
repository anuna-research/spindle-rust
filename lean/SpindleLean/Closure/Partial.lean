/-
  SpindleLean.Closure.Partial
  Partial closure: actual defeasible provability with conflict resolution.

  A literal p is defeasibly provable (+d) if:
  1. p is in Delta (already definite), OR
  2. ~p is NOT in Delta, AND
     there exists a supporting strict/defeasible rule with body in Partial, AND
     every attacker (rule with ~p in head) is either:
       - inapplicable (body not in Lambda), OR
       - defeated by a superior defender (team defeat)

  This implements skeptical/ambiguity-blocking semantics.
-/
import SpindleLean.Closure.Lambda

namespace Closure

/-- Check if an attacking rule's body can potentially be satisfied (is in lambda) -/
def attackReaches (lambda : List Literal) (attacker : Rule) : Bool :=
  attacker.body.all (lambda.contains ·)

/-- Check if there exists a defender for lit that is superior to the attacker -/
def teamDefeats (t : Theory) (lit : Literal) (attacker : Rule)
    (partial_ : List Literal) : Bool :=
  (t.rulesWithHead lit).any fun defender =>
    defender.isProductive
    && defender.bodySatisfied partial_
    && t.isSuperior defender.label attacker.label

/-- Check if all attacks against a literal are defeated -/
def allAttacksDefeated (t : Theory) (lit : Literal)
    (lambda partial_ : List Literal) : Bool :=
  let complement := lit.complement
  let attackers := t.rulesWithHead complement
  attackers.all fun attacker =>
    attacker.isFact  -- facts can't attack
    || !attackReaches lambda attacker  -- attack body unreachable
    || teamDefeats t lit attacker partial_  -- superior defender exists

/-- Check if a literal can be proven defeasibly -/
def canProve (t : Theory) (lit : Literal)
    (delta lambda partial_ : List Literal) : Bool :=
  -- Already in delta
  if delta.contains lit then true
  -- Complement in delta blocks
  else if delta.contains lit.complement then false
  -- Need a supporting rule with body satisfied in partial
  else
    let hasSupport := (t.rulesWithHead lit).any fun r =>
      r.isProductive && r.bodySatisfied partial_
    hasSupport && allAttacksDefeated t lit lambda partial_

/-- One step of partial closure -/
def partialStep (t : Theory) (delta lambda current : List Literal) : List Literal :=
  let candidates := lambda.filter fun lit =>
    !current.contains lit && canProve t lit delta lambda current
  (current ++ candidates).dedup

/-- Compute partial closure by iterating to fixpoint -/
def partialClose (t : Theory) (delta lambda : List Literal)
    (fuel : Nat := 1000) : List Literal :=
  go t delta lambda delta fuel
where
  go (t : Theory) (delta lambda current : List Literal) : Nat → List Literal
    | 0 => current
    | fuel + 1 =>
      let next := partialStep t delta lambda current
      if next.length == current.length then current
      else go t delta lambda next fuel

end Closure
