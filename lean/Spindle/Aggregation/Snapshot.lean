import Spindle.Aggregation.Fold
import Spindle.Aggregation.Stratification
import SpindleLean.Reason

/-!
# Completed conclusions at a stratum boundary

The caller supplies the final conclusions of the lower stratum. Candidate heads
from grounding are not conclusions. This module does not prove the caller has
reached that fixed point; it specifies selection and transport once it has.
-/
namespace Spindle.Aggregation

def positive (c : Conclusion) : Bool :=
  match c.conclusionType with
  | .definitelyProvable | .defeasiblyProvable => true
  | .definitelyNotProvable | .defeasiblyNotProvable => false

/-- One row per ground literal, even when both +D and +d prove it. -/
def snapshot (conclusions : List Conclusion) : List Literal :=
  ((conclusions.filter positive).map Conclusion.literal).dedup

theorem mem_snapshot (conclusions : List Conclusion) (literal : Literal) :
    literal ∈ snapshot conclusions ↔
      ∃ c ∈ conclusions, positive c = true ∧ c.literal = literal := by
  simp [snapshot, List.mem_filter, and_assoc]

theorem unproved_excluded (conclusions : List Conclusion) (literal : Literal)
    (unproved : ∀ c ∈ conclusions, c.literal = literal → positive c = false) :
    literal ∉ snapshot conclusions := by
  intro member
  obtain ⟨c, hc, hp, hl⟩ := (mem_snapshot conclusions literal).mp member
  have hn := unproved c hc hl
  simp [hn] at hp

/-- Carry proof strength into later strata without turning +d into +D.
The label is provenance supplied by the caller, not a new source of support. -/
def carry (label : String) (c : Conclusion) : Option Rule :=
  match c.conclusionType with
  | .definitelyProvable => some (Rule.fact label c.literal)
  | .defeasiblyProvable => some (Rule.defeasible label [] c.literal)
  | .definitelyNotProvable | .defeasiblyNotProvable => none

theorem carry_defeasible (label : String) (literal : Literal) :
    carry label ⟨literal, .defeasiblyProvable⟩ =
      some (Rule.defeasible label [] literal) := rfl

theorem carry_fact_iff (label : String) (c : Conclusion) (rule : Rule)
    (carried : carry label c = some rule) :
    rule.ruleType = .fact ↔ c.conclusionType = .definitelyProvable := by
  cases c with
  | mk literal kind =>
    cases kind <;> simp [carry, Rule.fact, Rule.defeasible] at carried ⊢
    all_goals subst rule; simp

/-- Future rows cannot alter an aggregate over the frozen input view. -/
theorem aggregate_frozen {Row Rel α : Type} [DecidableEq Row]
    (reducer : Reducer α) (seed : Option α) (owner : Row → Rel)
    (stratum : Rel → Nat) (consumer : Nat) (past future : List Row)
    (select : Row → Bool) (extract : Row → α)
    (later : ∀ row ∈ future, consumer ≤ stratum (owner row)) :
    reducer.eval seed (values (frozenRows owner stratum consumer (past ++ future)) select extract) =
      reducer.eval seed (values (frozenRows owner stratum consumer past) select extract) := by
  rw [frozenRows_append_future owner stratum consumer past future later]

-- Regressions corresponding to the proposed Rust implementation's failures.
example : snapshot [⟨Literal.pos "pay", .definitelyNotProvable⟩,
    ⟨Literal.pos "pay", .defeasiblyNotProvable⟩] = [] := by decide

example : snapshot [⟨Literal.pos "pay", .definitelyProvable⟩,
    ⟨Literal.pos "pay", .defeasiblyProvable⟩] = [Literal.pos "pay"] := by decide

example : (carry "pay-source" ⟨Literal.pos "pay", .defeasiblyProvable⟩).map Rule.ruleType =
    some .defeasible := rfl

end Spindle.Aggregation
