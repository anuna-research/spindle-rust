import Spindle.Aggregation.GroundBackend

namespace Spindle.Aggregation.GroundBackendTests

private def p := Literal.pos "p"
private def q := Literal.pos "q"
private def owner (literal : Literal) : Nat := if literal.name = "q" then 1 else 0

-- Ambiguous lower support is absent from +d, but still present in lambda.
-- The later defeater therefore remains applicable and blocks q.
private def ambiguous : Theory := ⟨[
  Rule.defeasible "p-support" [] p,
  Rule.defeasible "not-p-support" [] p.complement,
  Rule.defeasible "q-support" [] q,
  Rule.defeater "attack-q" [p] q.complement], []⟩

example : groundScheduled owner ambiguous = true := by decide
example : (reason (groundPrefix owner ambiguous 0)).containsLambda "p" = true := by decide
example : (reason (groundPrefix owner ambiguous 0)).containsPartial "p" = false := by decide

private def hasTag (conclusions : List Conclusion) (literal : Literal)
    (tag : ConclusionType) : Bool :=
  conclusions.any (fun c => c.literal == literal && c.conclusionType == tag)

private def finalHas (theory : Theory) (literal : Literal) (tag : ConclusionType) : Option Bool :=
  (executeStages owner (groundBackend owner theory) 2).toOption.map
    (fun state => hasTag state.conclusions literal tag)

example : finalHas ambiguous q .defeasiblyProvable = some false := by decide
example : finalHas ambiguous q .defeasiblyNotProvable = some true := by decide

-- Reconstructing just positive lower conclusions loses potential support.
-- This intentionally incorrect comparison backend demonstrates the hazard.
private def lossy : Theory :=
  ⟨(reason (groundPrefix owner ambiguous 0)).conclusions.filterMap (carry "carried") ++
    ambiguous.rules.filter (fun rule => decide (owner rule.head = 1)), ambiguous.superiority⟩

example : (reason lossy).containsPartial "q" = true := by decide

-- Strict consumption of defeasible support cannot turn the result into +D.
private def supported : Theory := ⟨[
  Rule.defeasible "support" [] p, Rule.strict "consume" [p] q], []⟩
example : finalHas supported p .defeasiblyProvable = some true := by decide
example : finalHas supported q .defeasiblyProvable = some true := by decide
example : finalHas supported q .definitelyProvable = some false := by decide

-- Priorities and their original rule labels survive prefix replay.
private def priorityResolved : Theory :=
  { ambiguous with superiority := [("p-support", "not-p-support"), ("q-support", "attack-q")] }
example : finalHas priorityResolved p .defeasiblyProvable = some true := by decide
example : finalHas priorityResolved q .defeasiblyProvable = some true := by decide

-- Fixed-point completion includes multiple local rounds and ordinary recursion.
private def recursive : Theory := ⟨[
  Rule.fact "seed" (Literal.pos "a"),
  Rule.strict "ab" [Literal.pos "a"] (Literal.pos "b"),
  Rule.strict "bc" [Literal.pos "b"] (Literal.pos "c"),
  Rule.strict "ca" [Literal.pos "c"] (Literal.pos "a"),
  Rule.strict "cq" [Literal.pos "c"] q], []⟩
example : finalHas recursive q .definitelyProvable = some true := by decide

-- A fold reads the actual completed reasoning output: defeated p contributes 0.
example : ((executeStages owner (groundBackend owner ambiguous) 2).toOption.bind
    (fun state => foldAt owner 1 state.conclusions sum (some 0)
      (fun literal => literal == p) (fun _ => 25))) = some 0 := by decide

-- Reject backward ordinary dependencies, split complements, and malformed facts.
example : groundScheduled owner ⟨[Rule.strict "backward" [q] p], []⟩ = false := by decide
example : groundScheduled (fun literal => if literal.negated then 1 else 0)
    ambiguous = false := by decide
example : groundScheduled owner ⟨[⟨"bad-fact", .fact, [p], p⟩], []⟩ = false := by decide
example : ((executeStages owner (groundBackend owner ⟨[Rule.strict "backward" [q] p], []⟩)
    2).toOption.map StageState.next) = none := by decide

-- A body-only lower literal must receive negative tags even before the rule
-- mentioning it is in the prefix. Reporting uses the full finite literal set.
private def bodyOnly : Theory := ⟨[Rule.strict "future-consumer" [p] q], []⟩
example : hasTag (reasonedStage owner bodyOnly 0) p .definitelyNotProvable = true := by decide
example : hasTag (reasonedStage owner bodyOnly 0) p .defeasiblyNotProvable = true := by decide

end Spindle.Aggregation.GroundBackendTests
