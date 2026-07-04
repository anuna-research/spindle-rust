/-
  SpindleLean.Closure.Partial
  Partial closure: actual defeasible provability with conflict resolution.

  A literal p is defeasibly provable (+d) if ~p is NOT in Delta, AND either:
  1. p is in Delta (already definite), OR
  2. there exists a supporting strict/defeasible rule with body in Partial, AND
     every attacker (rule with ~p in head) is either:
       - inapplicable (body not in Lambda), OR
       - defeated by a superior defender (team defeat)

  The `~p ∉ Delta` gate applies to BOTH clauses (spec condition (2),
  DEFEASIBLE-LOGIC-SEMANTICS.md worked example 1): when the definite level
  is itself contradictory (+D p and +D ~p), neither literal is defeasibly
  provable — strict contradictions are quarantined rather than propagated.
  This is a deliberate paraconsistent deviation from Antoniou et al., and
  it makes the defeasible level unconditionally consistent (up to
  superiority cycles).

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

/-- Check if a literal can be proven defeasibly.

    The complement-in-delta check comes FIRST: it gates the delta-subsumption
    clause as well as the rule clause (spec condition (2)). -/
def canProve (t : Theory) (lit : Literal)
    (delta lambda partial_ : List Literal) : Bool :=
  -- Complement in delta blocks everything (condition (2), gating subsumption)
  if delta.contains lit.complement then false
  -- Already in delta (and complement is not): subsumption
  else if delta.contains lit then true
  -- Need a supporting rule with body satisfied in partial
  else
    let hasSupport := (t.rulesWithHead lit).any fun r =>
      r.isProductive && r.bodySatisfied partial_
    hasSupport && allAttacksDefeated t lit lambda partial_

/-- The delta-consistent seed for partial closure: definite literals whose
    complement is not also definite. -/
def gatedDelta (delta : List Literal) : List Literal :=
  delta.filter fun l => !delta.contains l.complement

/-- One step of partial closure -/
def partialStep (t : Theory) (delta lambda current : List Literal) : List Literal :=
  let candidates := lambda.filter fun lit =>
    !current.contains lit && canProve t lit delta lambda current
  (current ++ candidates).dedup

/-- Compute partial closure by iterating to fixpoint -/
def partialClose (t : Theory) (delta lambda : List Literal)
    (fuel : Nat := 1000) : List Literal :=
  go t delta lambda (gatedDelta delta) fuel
where
  go (t : Theory) (delta lambda current : List Literal) : Nat → List Literal
    | 0 => current
    | fuel + 1 =>
      let next := partialStep t delta lambda current
      if next.length == current.length then current
      else go t delta lambda next fuel

end Closure
