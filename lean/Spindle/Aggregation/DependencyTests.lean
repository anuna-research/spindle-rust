import Spindle.Aggregation.Inference

/-! Kernel-checked examples from actual schema constructors. These exercise the
extractor, not hand-written graphs that could bypass missing dependencies. -/
namespace Spindle.Aggregation.DependencyTests

private def atom (name : String) : Pattern := ⟨⟨name, false, [.variable "x"]⟩, none, false⟩

private def ordinary (label source target : String) : SchemaRule :=
  ⟨label, .defeasible, [.logic (atom source)], atom target, []⟩

private def fold (label source target : String) : SchemaRule :=
  ⟨label, .defeasible, [.fold ⟨"total", some (.term (.integer 0)), "+",
    .term (.variable "x"), atom source, []⟩], atom target, []⟩

private def chain : AggregateProgram :=
  ⟨[ordinary "derive-a" "raw" "a", fold "sum-a" "a" "b",
    ordinary "derive-c" "b" "c", fold "sum-c" "c" "total"], []⟩

private def chainStages : DependencyNode → Nat
  | .predicate key => if key.name = "b" ∨ key.name = "c" then 1
      else if key.name = "total" then 2 else 0
  | .rule label => if label = "sum-a" ∨ label = "derive-c" then 1
      else if label = "sum-c" then 2 else 0

example : schemasRecognized chain = true := by decide
example : checkStrata (dependencies chain) chainStages = true := by decide

-- The second fold cannot run alongside its transitive input producers.
example : checkStrata (dependencies chain) (fun node => min (chainStages node) 1) = false := by
  decide

private def cycle : AggregateProgram :=
  ⟨[fold "sum-a" "a" "b", ordinary "back" "b" "a"], []⟩

-- Not merely failure of one candidate assignment: NO assignment works.
example : ¬ ∃ stage, checkStrata (dependencies cycle) stage = true := by
  rintro ⟨stage, accepted⟩
  have scheduled := (checkProgram_iff cycle stage).mp accepted
  have first := scheduled (fold "sum-a" "a" "b") (by simp [cycle])
  have second := scheduled (ordinary "back" "b" "a") (by simp [cycle])
  have hb := first.1 (atom "b") (by simp [fold, SchemaRule.heads])
  have ha := second.1 (atom "a") (by simp [ordinary, SchemaRule.heads])
  have forward := first.2 (.fold ⟨"total", some (.term (.integer 0)), "+",
    .term (.variable "x"), atom "a", []⟩) (by simp [fold]) (atom "a") true rfl
  have backward := second.2 (.logic (atom "b")) (by simp [ordinary]) (atom "b") false rfl
  simp only [fold, ordinary] at hb ha forward backward
  simp only [ite_true, Bool.false_eq_true, ite_false, Nat.add_zero] at forward backward
  omega

-- Positive recursion remains legal and has no artificial aggregate barrier.
example : checkStrata (dependencies ⟨[ordinary "ab" "a" "b",
    ordinary "ba" "b" "a"], []⟩) (fun _ => 0) = true := by decide

private def attack : SchemaRule :=
  { ordinary "attack" "signal" "pay" with
    kind := .defeater
    head := (atom "pay").complement }

private def attacked : AggregateProgram :=
  ⟨[ordinary "pay-rule" "raw" "pay", attack, fold "total-rule" "pay" "total"],
    [("attack", "pay-rule")]⟩

private def attackedStages : DependencyNode → Nat
  | .predicate key => if key.name = "total" then 1 else 0
  | .rule label => if label = "total-rule" then 1 else 0

example : schemasRecognized attacked = true := by decide
example : checkStrata (dependencies attacked) attackedStages = true := by decide

-- The defeater's applicability cannot be postponed beyond the pay domain.
example : checkStrata (dependencies attacked) (fun node =>
    if node = .rule "attack" then 1 else attackedStages node) = false := by decide

-- An attack depending on the aggregate itself is a cycle even though its head
-- is negated and its rule kind is a defeater.
private def aggregateAttack : AggregateProgram :=
  ⟨[fold "total-rule" "pay" "total",
    { attack with body := [.logic (atom "total")] }], []⟩

example : checkStrata (dependencies aggregateAttack) attackedStages = false := by decide

-- Modal variants share a conservative conflict domain.
example : ({ atom "pay" with modeName := some "O" } : Pattern).key =
    ({ (atom "pay").complement with modeName := some "P" } : Pattern).key := rfl

-- All heads of a single rule must have the rule's stratum.
example : checkStrata (dependencies ⟨[{ ordinary "both" "raw" "a" with
    additionalHeads := [atom "b"] }], []⟩)
    (fun node => if node = .predicate (atom "b").key then 1 else 0) = false := by decide

-- Priority between unrelated heads has no scheduling effect.
example : checkStrata (dependencies { chain with priorities := [("sum-c", "derive-a")] })
    chainStages = true := by decide

-- Different arities remain distinct dependency domains.
example : (atom "pay").key ≠ (⟨⟨"pay", false, []⟩, none, false⟩ : Pattern).key := by decide

-- Unsupported schematic functors, duplicate labels, and unknown priority
-- references are rejected rather than silently losing dependencies.
example : schemasRecognized ⟨[ordinary "dynamic" "?predicate" "a"], []⟩ = false := by decide
example : schemasRecognized ⟨[ordinary "wildcard" "_" "a"], []⟩ = false := by decide
example : schemasRecognized ⟨[ordinary "private-name" "_source" "a"], []⟩ = true := by decide
example : schemasRecognized ⟨[ordinary "duplicate" "a" "b",
    ordinary "duplicate" "b" "c"], []⟩ = false := by decide
example : schemasRecognized { chain with priorities := [("unknown", "sum-a")] } = false := by decide

-- Kernel-checked automatic inference for elementary graphs.
example : (inferStrata ([] : List (Dependency Nat))).map (fun stage => stage 42) = some 0 := by
  decide

example : (inferStrata ([⟨0, 1, false⟩, ⟨1, 0, false⟩] : List (Dependency Nat))).map
    (fun stage => [stage 0, stage 1]) = some [0, 0] := by decide

example : (inferStrata ([⟨0, 1, true⟩, ⟨1, 2, false⟩, ⟨2, 3, true⟩] :
    List (Dependency Nat))).map (fun stage => [stage 0, stage 1, stage 2, stage 3]) =
      some [0, 1, 1, 2] := by decide

-- Runtime assertions also execute larger schema graphs through the actual
-- cache and solver, including rejection after the graph-derived iteration bound.
-- They are smoke tests, not premises or proofs via native_decide.
private def stagesFor (program : AggregateProgram) (labels : List String) :
    Except InferenceError (List Nat) :=
  (inferProgram program).map (fun stage => labels.map (fun label => stage (.rule label)))

private def sameResult (actual expected : Except InferenceError (List Nat)) : Bool :=
  match actual, expected with
  | .ok a, .ok b => decide (a = b)
  | .error a, .error b => decide (a = b)
  | _, _ => false

#eval show IO Unit from do
  unless sameResult (stagesFor chain ["derive-a", "sum-a", "derive-c", "sum-c"]) (.ok [0, 1, 1, 2]) do
    throw (IO.userError "inference failed on a transitive aggregate chain")
  unless sameResult (stagesFor { chain with rules := chain.rules.reverse } ["derive-a", "sum-a", "derive-c", "sum-c"]) (.ok [0, 1, 1, 2]) do
    throw (IO.userError "inference depends on rule ordering")
  unless sameResult (stagesFor cycle []) (.error .unstratifiable) do
    throw (IO.userError "inference accepted an indirect aggregate cycle")
  unless sameResult (stagesFor attacked ["pay-rule", "attack", "total-rule"]) (.ok [0, 0, 1]) do
    throw (IO.userError "inference did not finalize the attacker before the fold")
  unless sameResult (stagesFor aggregateAttack []) (.error .unstratifiable) do
    throw (IO.userError "inference missed a cycle through a defeater")
  let multi : AggregateProgram := ⟨[{ ordinary "both" "raw" "a" with
      additionalHeads := [atom "b"] }, fold "seed-b" "seed" "b"], []⟩
  unless sameResult (stagesFor multi ["both", "seed-b"]) (.ok [1, 1]) do
    throw (IO.userError "inference split the heads of one rule across strata")
  unless sameResult (stagesFor ⟨[ordinary "dynamic" "?predicate" "a"], []⟩ []) (.error .unrecognizedSchemas) do
    throw (IO.userError "unsupported schemas were confused with an aggregate cycle")
  unless sameResult (stagesFor { chain with priorities := [("sum-c", "derive-a")] }
      ["derive-a", "sum-c"]) (.ok [0, 2]) do
    throw (IO.userError "unrelated priority changed the inferred strata")

end Spindle.Aggregation.DependencyTests
