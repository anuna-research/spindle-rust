import Spindle.Aggregation.Execution

namespace Spindle.Aggregation.ExecutionTests

private def owner (literal : Literal) : Nat := if literal.name = "later" then 1 else 0
private def initial : StageState := ⟨0, []⟩

-- Fixture batches stand in for a completed local reasoner. Duplicate positive
-- evidence contributes once; a negatively tagged row contributes nothing.
private def backend : StageBackend := fun stage _ => .ok (if stage = 0 then
  [⟨Literal.pos "p", .defeasiblyProvable⟩,
   ⟨Literal.pos "p", .defeasiblyProvable⟩,
   ⟨Literal.pos "q", .definitelyProvable⟩,
   ⟨Literal.pos "r", .defeasiblyNotProvable⟩]
  else [⟨Literal.pos "later", .defeasiblyProvable⟩])

private def total (consumer : Nat) (state : StageState) : Option Int :=
  foldAt owner consumer state.conclusions sum (some 0) (fun _ => true) (fun _ => 25)

example : ((runStages owner backend 2 initial).toOption.map StageState.next) = some 2 := by decide
example : ((runStages owner backend 2 initial).toOption.bind (total 1)) = some 50 := by decide
example : ((runStages owner backend 2 initial).toOption.bind (total 2)) = some 75 := by decide

-- A later callback actually observes the committed, deduplicated lower rows.
private def readingBackend : StageBackend := fun stage prior =>
  if stage = 0 then backend stage prior
  else if foldAt owner stage prior sum (some 0) (fun _ => true) (fun _ => 25) = some 50 then
    .ok []
  else .error "incorrect aggregate snapshot"

example : ((runStages owner readingBackend 2 initial).toOption.map StageState.next) = some 2 := by
  decide

private def failure (result : Except ExecutionError StageState) : Option ExecutionError :=
  match result with
  | .ok _ => none
  | .error error => some error

-- Reject both premature publication and attempts to revise a completed domain.
example : failure (runStages owner (fun _ _ => .ok [⟨Literal.pos "later", .definitelyProvable⟩])
    1 initial) = some (.wrongOwner 0) := by decide
example : failure (runStages owner (fun _ _ => .ok [⟨Literal.pos "p", .definitelyProvable⟩])
    1 ⟨1, [⟨Literal.pos "p", .defeasiblyProvable⟩]⟩) = some (.wrongOwner 1) := by decide

example : failure (runStages owner (fun _ _ => .error "local limit") 2 initial) =
    some (.backend "local limit") := by decide
example : ((runStages owner (fun _ _ => .error "must not run") 0 initial).toOption.map
    StageState.next) = some 0 := by decide

-- Empty required folds fail; an explicit seed survives an empty completed stage.
example : foldAt owner 1 [] minimum none (fun _ => true) (fun _ => 25) = none := rfl
example : foldAt owner 1 [] sum (some 0) (fun _ => true) (fun _ => 25) = some 0 := rfl

-- Definite membership of all included rows does not make the exact sum stable
-- under extensions to the input domain. Closure is a separate dependency.
example : foldAt owner 1 [⟨Literal.pos "p", .definitelyProvable⟩]
    sum (some 0) (fun _ => true) (fun _ => 25) = some 25 := by decide
example : foldAt owner 1 [⟨Literal.pos "p", .definitelyProvable⟩,
    ⟨Literal.pos "q", .definitelyProvable⟩]
    sum (some 0) (fun _ => true) (fun _ => 25) = some 50 := by decide

end Spindle.Aggregation.ExecutionTests
