import Spindle.Aggregation.LoweringCorrectness

namespace Spindle.Aggregation.LoweringCorrectnessTests

private def row (id : Int) : Pattern := ⟨⟨"shift", false, [.integer id]⟩, none, false⟩
private def rows : List Pattern := [row 1, row 2, row 1]
private def seeded : FoldExpression :=
  ⟨"n", some (.term (.integer 7)), "+", .term (.integer 25),
    ⟨⟨"shift", false, [.variable "id"]⟩, none, false⟩, []⟩

-- A direct source derivation: reversed row enumeration, duplicate removed,
-- both equal contributions retained, and a non-identity seed included once.
private theorem source_two_rows (σ : Arith.Substitution) (unbound : σ.lookup "id" = none) :
    Source.Fold [.integer 57] σ rows seeded (some 57) := by
  refine ⟨sum, some 7, [25, 25], .add, .present (by simp [evalExpr, Arith.Substitution.applyTerm]), ?_, ?_, ?_⟩
  · refine ⟨[row 2, row 1], by decide, ?_, ?_⟩
    · intro p
      simp only [rows, List.mem_cons, List.not_mem_nil, or_false]
      tauto
    · change List.Forall₂ _
        ([row 2, row 1].filterMap (matchRow σ seeded.pattern)) [25, 25]
      simp only [List.filterMap_cons, List.filterMap_nil, matchRow, row, seeded,
        Arith.matchTerms, Arith.matchTerm, unbound]
      exact .cons (by simp [evalExpr, Arith.Substitution.applyTerm])
        (.cons (by simp [evalExpr, Arith.Substitution.applyTerm]) .nil)
  · exact .next (.refl _) (.next (.refl _) (.empty _))
  · intro n hn
    cases hn
    simp

-- Transport the source proof to executable evaluation, exercising completeness.
example : evalSchemaFold [.integer 57] Arith.Substitution.empty rows seeded = .ok (some 57) :=
  (evalSchemaFold_iff _ _ _ _ _).mpr (source_two_rows _ rfl)

example : Source.Fold [.integer 57] Arith.Substitution.empty [row 2, row 1] seeded (some 57) := by
  apply (Source.Fold.rows_extensional _ _ rows [row 2, row 1] _ _ ?_).mp (source_two_rows _ rfl)
  intro p
  simp only [rows, List.mem_cons, List.not_mem_nil, or_false]
  tauto

example : Source.Fold [] Arith.Substitution.empty [] { seeded with seed := none } none := by
  exact ⟨sum, none, [], .add, .absent, ⟨[], .nil, fun _ => Iff.rfl, .nil⟩,
    .empty none, fun _ h => nomatch h⟩

example : Source.Fold [.integer 7] Arith.Substitution.empty [] seeded (some 7) := by
  refine ⟨sum, some 7, [], .add, .present (by simp [evalExpr, Arith.Substitution.applyTerm]), ⟨[], .nil, fun _ => Iff.rfl, .nil⟩,
    .empty _, ?_⟩
  intro n hn
  cases hn
  simp

example : ¬Source.Fold [.integer 25] Arith.Substitution.empty rows seeded (some 57) := by
  rintro ⟨_, _, _, _, _, _, _, bounded⟩
  have impossible := bounded 57 rfl
  contradiction

private def atom (name : String) : Pattern := ⟨⟨name, false, []⟩, none, false⟩
private def schema : SchemaRule :=
  ⟨"total", .strict, [.fold seeded, .logic (atom "gate")], atom "total", [atom "copy"]⟩
private def env : Arith.Substitution := Arith.Substitution.empty.set "n" (.integer 57)
private def table : List Pattern := [atom "gate", atom "total", atom "copy"]

-- Ordinary gate truth is not required to emit the rule with its residual premise.
private theorem enabled : Source.Enabled [.integer 57] env rows schema := by
  intro c hc
  simp only [schema, List.mem_cons, List.not_mem_nil, or_false] at hc
  rcases hc with rfl | rfl
  · exact Source.ConditionValue.fold (source_two_rows env rfl)
  · exact .logic _

-- The completeness premises are inhabited for a strict, multi-head aggregate.
example : ∃ fresh, lowerInstance .defeasibleEvidence [.integer 57] table 1 rows schema env = .ok fresh ∧
    ∀ rule, rule ∈ fresh ↔ SourceRule [.integer 57] table 1 rows schema env rule := by
  apply lowerInstance_complete _ _ _ _ _ _ _ enabled
  · simp
  · intro p hp
    simp only [schema, SchemaRule.heads, SchemaRule.premises, List.filterMap_cons,
      List.filterMap_nil, List.mem_append, List.mem_cons, List.not_mem_nil, or_false] at hp
    rcases hp with (rfl | rfl) | rfl <;> decide

end Spindle.Aggregation.LoweringCorrectnessTests
