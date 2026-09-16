import Spindle.Aggregation.Lowering

namespace Spindle.Aggregation.LoweringTests

private def atom (name : String) (args : List Arith.Term) : Pattern := ⟨⟨name, false, args⟩, none, false⟩
private def domain : Arith.Domain := [.integer 0, .integer 1, .integer 2, .integer 25, .integer 50]
private def shift (label : String) (id : Int) : SchemaRule :=
  ⟨label, .defeasible, [], atom "shift" [.integer id, .integer 25], []⟩
private def totalFold : FoldExpression :=
  ⟨"n", some (.term (.integer 0)), "+", .term (.variable "pay"),
    atom "shift" [.variable "id", .variable "pay"], []⟩
private def total : SchemaRule :=
  ⟨"total", .strict, [.fold totalFold], atom "total" [.variable "n"], []⟩
private def program : AggregateProgram := ⟨[shift "one" 1, shift "two" 2, shift "duplicate" 1, total], []⟩

private def observed (p : AggregateProgram) (d : Arith.Domain) (query : Pattern) : Except String (Bool × Bool) := do
  let (lowered, state) ← evaluateProgram (program := p) (domain := d)
  let literal := encodeAtom lowered.table query
  return (state.conclusions.any (fun c => c.literal == literal && c.conclusionType == .definitelyProvable),
    state.conclusions.any (fun c => c.literal == literal && c.conclusionType == .defeasiblyProvable))

-- Integration assertions execute inferred scheduling, grounding, row selection,
-- fold reduction, guard lowering, and the real three-phase reasoner.
#eval show IO Unit from do
  match observed program domain (atom "total" [.integer 50]) with
  | .ok (false, true) => pure ()
  | _ => throw (IO.userError "distinct equal contributions or aggregate proof strength changed")
  match observed { program with rules := program.rules.reverse } domain (atom "total" [.integer 50]) with
  | .ok (false, true) => pure ()
  | _ => throw (IO.userError "schema ordering changed the lowered result")
  let multi := { total with additionalHeads := [atom "copy" [.variable "n"]] }
  match observed { program with rules := [shift "one" 1, shift "two" 2, multi] }
      domain (atom "copy" [.integer 50]) with
  | .ok (false, true) => pure ()
  | _ => throw (IO.userError "multi-head aggregate lowering lost an output")
  let grouped := { total with label := "grouped", head := atom "grouped" [.variable "id", .variable "n"], body := [.fold { totalFold with groupingVars := ["id"] }] }
  match observed { program with rules := [shift "one" 1, shift "two" 2, grouped] }
      domain (atom "grouped" [.integer 1, .integer 25]) with
  | .ok (false, true) => pure ()
  | _ => throw (IO.userError "outer group bindings were not respected")
  let computed := { total with head := atom "double" [.variable "m"], body := [.fold totalFold,
      .bind "m" (.call "*" [.term (.variable "n"), .term (.integer 2)]),
      .compare .gt (.term (.variable "m")) (.term (.variable "n"))] }
  let wider := domain ++ [.integer 100]
  match observed { program with rules := [shift "one" 1, shift "two" 2, computed] }
      wider (atom "double" [.integer 100]) with
  | .ok (false, true) => pure ()
  | _ => throw (IO.userError "binds or comparisons after a fold were lowered incorrectly")
  let definiteInputs := { program with rules := [{ shift "one" 1 with kind := .fact },
      { shift "two" 2 with kind := .fact }, total] }
  match observed definiteInputs wider (atom "total" [.integer 50]) with
  | .ok (false, true) => pure ()
  | _ => throw (IO.userError "dual +D/+d evidence was counted twice or promoted aggregate strength")
  let attack : SchemaRule := ⟨"attack", .defeater, [],
    (atom "shift" [.integer 1, .integer 25]).complement, []⟩
  match observed { program with rules := program.rules ++ [attack] } domain (atom "total" [.integer 25]) with
  | .ok (false, true) => pure ()
  | _ => throw (IO.userError "a defeated shift entered the aggregate")
  let prioritized := { program with rules := program.rules ++ [attack], priorities := [("one", "attack")] }
  match observed prioritized domain (atom "total" [.integer 50]) with
  | .ok (false, true) => pure ()
  | _ => throw (IO.userError "schema priorities were lost during lowering")
  let grand : SchemaRule := ⟨"grand", .defeasible,
    [.fold { totalFold with pattern := atom "total" [.variable "pay"] }],
    atom "grand" [.variable "n"], []⟩
  match observed { program with rules := program.rules ++ [grand] } domain (atom "grand" [.integer 50]) with
  | .ok (false, true) => pure ()
  | _ => throw (IO.userError "aggregate chain failed across completed strata")
  match lowerProgram .rejectStrict program domain with
  | .error _ => pure ()
  | .ok _ => throw (IO.userError "strict aggregates escaped the rejection policy")
  match lowerProgram (program := program) (domain := [.integer 0, .integer 1, .integer 2, .integer 25]) with
  | .error _ => pure ()
  | .ok _ => throw (IO.userError "an out-of-domain aggregate result was silently lost")
  match observed ⟨[total], []⟩ domain (atom "total" [.integer 0]) with
  | .ok (false, true) => pure ()
  | _ => throw (IO.userError "empty seeded aggregate did not retain its seed")
  let required := { total with body := [.fold { totalFold with seed := none, reducer := "min" }] }
  match observed ⟨[required], []⟩ domain (atom "total" [.integer 0]) with
  | .ok (false, false) => pure ()
  | _ => throw (IO.userError "empty required aggregate produced a value")
  let unknown := { total with body := [.fold { totalFold with reducer := "unknown" }] }
  match lowerProgram .defeasibleEvidence ⟨[unknown], []⟩ [] with
  | .error _ => pure ()
  | .ok _ => throw (IO.userError "unsupported reducers escaped recognition on an empty domain")
  let cycle := { total with head := atom "shift" [.variable "n", .integer 25] }
  match lowerProgram .defeasibleEvidence ⟨[cycle], []⟩ domain with
  | .error _ => pure ()
  | .ok _ => throw (IO.userError "aggregate cycles reached execution")

end Spindle.Aggregation.LoweringTests
