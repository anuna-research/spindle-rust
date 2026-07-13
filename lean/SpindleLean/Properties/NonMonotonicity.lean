/-
  SpindleLean.Properties.NonMonotonicity
  Machine-checked witness that defeasible reasoning is NOT monotone.

  Adding rules to a theory can RETRACT defeasible conclusions:

      T  = { a => p,  >> a }        derives +d p
      T' = T ∪ { >> ~p }            no longer derives +d p

  This is the reviewer-facing counterexample for the query layer: any
  contract that demands "adding rules preserves every conclusion"
  (`Spindle.Arith.ReasoningOp.mono`) is NOT satisfied by the defeasible
  conclusions of the engine or of this model. The monotone query theorems
  (`whatIf_mono`, `abduce_solution_superset`, ...) therefore apply to the
  monotone SUPPORT closure (`Spindle.Arith.supportOp`), never to `+d`.

  All three facts below are established by kernel computation (`decide`)
  on the executable three-phase model — no axioms beyond the standard
  ones.
-/
import SpindleLean.Reason

namespace Properties

/-- `T = { a => p, >> a }`. -/
def nonMonoBase : Theory :=
  ⟨[Rule.defeasible "r0" [Literal.pos "a"] (Literal.pos "p"),
    Rule.fact "f0" (Literal.pos "a")], []⟩

/-- `T' = T ∪ { >> ~p }` — the hypothetical fact `~p` added on top. -/
def nonMonoExtended : Theory :=
  ⟨nonMonoBase.rules ++ [Rule.fact "f1" (Literal.neg "p")], []⟩

theorem nonMonoBase_rules_subset :
    ∀ r ∈ nonMonoBase.rules, r ∈ nonMonoExtended.rules := by
  intro r hr
  exact List.mem_append_left _ hr

/-- The base theory defeasibly derives `p`. -/
theorem nonMonoBase_derives_p :
    (reason nonMonoBase).containsPartial "p" = true := by decide

/-- The extended theory does NOT defeasibly derive `p`: the added fact
    `~p` is definite, and the delta-consistency gate blocks `+d p`. -/
theorem nonMonoExtended_not_derives_p :
    (reason nonMonoExtended).containsPartial "p" = false := by decide

/-- **Defeasible conclusions are not monotone in the theory.** There is a
    theory extension (rule-set superset) that loses a `+d` conclusion, so
    no `ReasoningOp`-style monotonicity contract can be instantiated by
    the defeasible level. -/
theorem defeasible_not_monotone :
    ¬ (∀ (t t' : Theory), (∀ r ∈ t.rules, r ∈ t'.rules) →
        ∀ name, (reason t).containsPartial name = true →
          (reason t').containsPartial name = true) := by
  intro h
  have hp := h nonMonoBase nonMonoExtended nonMonoBase_rules_subset "p"
    nonMonoBase_derives_p
  rw [nonMonoExtended_not_derives_p] at hp
  exact Bool.noConfusion hp

end Properties
