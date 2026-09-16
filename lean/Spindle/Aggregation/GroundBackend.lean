import Spindle.Aggregation.Execution
import Spindle.Aggregation.Operational

/-!
# Reference backend for finite ground rule prefixes

Re-run the complete rule prefix through all four constructive closures, then publish
only the current stage's conclusions. Retaining lower rules is essential:
positive conclusion transport alone loses the distinction between discarded
and undecided attackers.

This backend handles ordinary finite ground theories. It does not lower fold
schemas to rules. The ownership map is supplied. `PrefixEquivalence.lean` proves agreement with
full-theory reasoning; `Lowering.lean` connects it to inferred schema stages.
-/
namespace Spindle.Aggregation

/-- Check ordinary dependency ordering, complementary-domain alignment, and
empty fact bodies. This is not a complete parser or superiority validator. -/
def groundScheduled (owner : Literal → Nat) (theory : Theory) : Bool :=
  theory.rules.all (fun rule =>
    (if rule.ruleType = .fact then rule.body.isEmpty else true) &&
    rule.body.all (fun literal => decide (owner literal ≤ owner rule.head))) &&
  theory.allLiterals.all (fun literal => decide (owner literal.complement = owner literal))

theorem groundScheduled_iff (owner : Literal → Nat) (theory : Theory) :
    groundScheduled owner theory = true ↔
      theory.WellFormed ∧
      (∀ rule ∈ theory.rules, ∀ literal ∈ rule.body, owner literal ≤ owner rule.head) ∧
      (∀ literal ∈ theory.allLiterals, owner literal.complement = owner literal) := by
  simp only [groundScheduled, Bool.and_eq_true, List.all_eq_true, decide_eq_true_eq]
  constructor
  · rintro ⟨rules, complements⟩
    refine ⟨?_, fun rule member => (rules rule member).2, complements⟩
    intro rule member fact
    have empty := (rules rule member).1
    simpa [fact] using empty
  · rintro ⟨wellformed, bodies, complements⟩
    refine ⟨?_, complements⟩
    intro rule member
    refine ⟨?_, bodies rule member⟩
    split
    next fact => simp [wellformed rule member fact]
    · rfl

/-- Keep original rules and priorities, including defeated support and defeaters. -/
def groundPrefix (owner : Literal → Nat) (theory : Theory) (stage : Nat) : Theory :=
  { theory with rules := theory.rules.filter (fun rule => decide (owner rule.head ≤ stage)) }

/-- Report both proof levels over a fixed literal set. A literal mentioned only in
future rule bodies still receives its negative tags at its own stage. -/
def reportLiteral (result : Operational.Result) (literal : Literal) : List Conclusion :=
  result.report literal

theorem reportLiteral_owned (result : Operational.Result) (l : Literal) (c : Conclusion)
    (member : c ∈ reportLiteral result l) : c.literal = l := by
  obtain ⟨tag, _, h⟩ := List.mem_filterMap.mp member
  split at h
  · cases h
    rfl
  · cases h


def reportConclusions (literals : List Literal) (result : Operational.Result) : List Conclusion :=
  literals.flatMap (reportLiteral result)

theorem reason_reports (theory : Theory) :
    reportConclusions theory.allLiterals (Operational.reason theory) = (Operational.reason theory).conclusions := rfl

/-- Project a completed prefix's tagged conclusions onto its current domain. -/
def reasonedStage (owner : Literal → Nat) (theory : Theory) (stage : Nat) : List Conclusion :=
  (reportConclusions theory.allLiterals (Operational.reason (groundPrefix owner theory stage))).filter
    (fun c => decide (owner c.literal = stage))

theorem mem_reasonedStage (owner : Literal → Nat) (theory : Theory) (stage : Nat)
    (c : Conclusion) : c ∈ reasonedStage owner theory stage ↔
      c ∈ reportConclusions theory.allLiterals (Operational.reason (groundPrefix owner theory stage)) ∧ owner c.literal = stage := by
  simp [reasonedStage]

/-- Replay uses retained rules rather than reconstructing them from prior tags. -/
def groundBackend (owner : Literal → Nat) (theory : Theory) : StageBackend :=
  fun stage _ =>
    if groundScheduled owner theory then .ok (reasonedStage owner theory stage)
    else .error "ground theory has invalid stage dependencies or fact bodies"

/-- Each published batch comes from an actual completed four-tag run. -/
theorem groundBackend_completed (owner : Literal → Nat) (theory : Theory)
    (stage : Nat) (prior batch : List Conclusion)
    (returned : groundBackend owner theory stage prior = .ok batch) :
    let localTheory := groundPrefix owner theory stage
    let result := Operational.reason localTheory
    Operational.step localTheory result.state = result.state ∧
    (∀ c, c ∈ batch ↔ c ∈ reportConclusions theory.allLiterals result ∧ owner c.literal = stage) := by
  unfold groundBackend at returned
  split at returned
  · simp only [Except.ok.injEq] at returned
    subst batch
    exact ⟨Operational.close_fixedpoint _, mem_reasonedStage owner theory stage⟩
  · simp at returned

/-- Recognized finite ground theories always finish the requested stage count;
there is no arbitrary local fuel cutoff or synthetic wrong-owner failure. -/
theorem groundBackend_runs (owner : Literal → Nat) (theory : Theory)
    (recognized : groundScheduled owner theory = true) (count : Nat) (before : StageState) :
    ∃ after, runStages owner (groundBackend owner theory) count before = .ok after := by
  induction count generalizing before with
  | zero => exact ⟨before, rfl⟩
  | succ count ih =>
    have owned : (reasonedStage owner theory before.next).all
        (fun c => decide (owner c.literal = before.next)) = true := by
      apply List.all_eq_true.mpr
      intro c member
      exact decide_eq_true ((mem_reasonedStage owner theory before.next c).mp member).2
    simpa only [runStages, advanceStage, groundBackend, recognized, if_true, owned] using
      ih ⟨before.next + 1, before.conclusions ++ reasonedStage owner theory before.next⟩

end Spindle.Aggregation
