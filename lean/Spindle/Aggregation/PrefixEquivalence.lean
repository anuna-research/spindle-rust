import Spindle.Aggregation.GroundBackend
import SpindleLean.Properties.Soundness
import SpindleLean.Properties.Subset

namespace Spindle.Aggregation

/-- Inclusion restricted to a completed set of literal domains. -/
def Below (low : Literal → Prop) (xs ys : List Literal) : Prop :=
  ∀ l, low l → l ∈ xs → l ∈ ys

def Agree (low : Literal → Prop) (xs ys : List Literal) : Prop :=
  ∀ l, low l → (l ∈ xs ↔ l ∈ ys)

private theorem contains_agree (low : Literal → Prop) (xs ys : List Literal)
    (agree : Agree low xs ys) (l : Literal) (hl : low l) : xs.contains l = ys.contains l := by
  apply Bool.eq_iff_iff.mpr
  simpa only [List.contains_iff_mem] using agree l hl

/-- All premises of a rule in a completed domain also belong to completed domains. -/
def ClosedRules (low : Literal → Prop) (t : Theory) : Prop :=
  ∀ r ∈ t.rules, low r.head → ∀ l ∈ r.body, low l

def SameRules (low : Literal → Prop) (t u : Theory) : Prop :=
  ∀ r, low r.head → (r ∈ t.rules ↔ r ∈ u.rules)

private theorem body_below (low : Literal → Prop) (r : Rule) (xs ys : List Literal)
    (closed : ∀ l ∈ r.body, low l) (below : Below low xs ys)
    (body : r.bodySatisfied xs = true) : r.bodySatisfied ys = true := by
  simp only [Rule.bodySatisfied, List.all_eq_true, List.contains_iff_mem] at body ⊢
  exact fun l member => below l (closed l member) (body l member)

private theorem body_agree (low : Literal → Prop) (r : Rule) (xs ys : List Literal)
    (closed : ∀ l ∈ r.body, low l) (agree : Agree low xs ys) :
    r.bodySatisfied xs = r.bodySatisfied ys := by
  apply Bool.eq_iff_iff.mpr
  constructor
  · exact body_below low r xs ys closed (fun l hl => (agree l hl).mp)
  · exact body_below low r ys xs closed (fun l hl => (agree l hl).mpr)

private theorem delta_admits (t : Theory) (xs : List Literal) (r : Rule)
    (member : r ∈ t.rules) (definite : r.isDefinite = true)
    (body : r.bodySatisfied xs = true) : r.head ∈ Closure.deltaStep t xs := by
  by_cases old : r.head ∈ xs
  · exact Properties.mem_deltaStep_of_mem t xs r.head old
  · simp only [Closure.deltaStep, List.mem_dedup, List.mem_append, List.mem_filterMap]
    right
    exact ⟨r, member, by simp [definite, body, old]⟩

private theorem lambda_admits (t : Theory) (delta xs : List Literal) (r : Rule)
    (member : r ∈ t.rules) (productive : r.isProductive = true)
    (body : r.bodySatisfied xs = true) (opposite : delta.contains r.head.complement = false) :
    r.head ∈ Closure.lambdaStep t delta xs := by
  by_cases old : r.head ∈ xs
  · exact Properties.mem_lambdaStep_of_mem t delta xs r.head old
  · simp only [Closure.lambdaStep, List.mem_dedup, List.mem_append, List.mem_filterMap]
    right
    have absent : xs.contains r.head = false := by simpa using old
    exact ⟨r, member, by simp only [productive, body, opposite, absent, Bool.true_and,
      Bool.not_false, ite_true]⟩

private theorem partial_admits (t : Theory) (delta lambda xs : List Literal) (l : Literal)
    (candidate : l ∈ lambda) (can : Closure.canProve t l delta lambda xs = true) :
    l ∈ Closure.partialStep t delta lambda xs := by
  by_cases old : l ∈ xs
  · exact Properties.mem_partialStep_of_mem t delta lambda xs l old
  · simp [Closure.partialStep, candidate, can, old]

private theorem delta_step_below (low : Literal → Prop) (t u : Theory)
    (closed : ClosedRules low t) (same : SameRules low t u)
    (xs ys : List Literal) (below : Below low xs ys) :
    Below low (Closure.deltaStep t xs) (Closure.deltaStep u ys) := by
  intro l hl member
  by_cases old : l ∈ xs
  · exact Properties.mem_deltaStep_of_mem u ys l (below l hl old)
  · obtain ⟨r, hr, definite, head, body⟩ := Properties.deltaStep_new_has_rule t xs l member old
    subst l
    exact delta_admits u ys r ((same r hl).mp hr) definite
      (body_below low r xs ys (closed r hr hl) below body)

private theorem lambda_step_below (low : Literal → Prop) (t u : Theory)
    (closed : ClosedRules low t) (same : SameRules low t u)
    (complements : ∀ l, low l → low l.complement)
    (dt du xs ys : List Literal) (delta : Agree low dt du) (below : Below low xs ys) :
    Below low (Closure.lambdaStep t dt xs) (Closure.lambdaStep u du ys) := by
  intro l hl member
  simp only [Closure.lambdaStep, List.mem_dedup, List.mem_append, List.mem_filterMap] at member
  rcases member with old | ⟨r, hr, produced⟩
  · exact Properties.mem_lambdaStep_of_mem u du ys l (below l hl old)
  · split at produced
    next guard =>
      simp only [Option.some.injEq] at produced
      subst l
      simp only [Bool.and_eq_true, Bool.not_eq_true'] at guard
      apply lambda_admits u du ys r ((same r hl).mp hr) guard.1.1.1
        (body_below low r xs ys (closed r hr hl) below guard.1.1.2)
      rw [← contains_agree low dt du delta r.head.complement (complements r.head hl)]
      exact guard.2
    · simp at produced

private theorem team_below (low : Literal → Prop) (t u : Theory)
    (closed : ClosedRules low t) (same : SameRules low t u)
    (priorities : t.superiority = u.superiority)
    (xs ys : List Literal) (below : Below low xs ys) (l : Literal) (hl : low l) (attacker : Rule)
    (team : Closure.teamDefeats t l attacker xs = true) :
    Closure.teamDefeats u l attacker ys = true := by
  simp only [Closure.teamDefeats, List.any_eq_true, Theory.rulesWithHead, List.mem_filter,
    beq_iff_eq, Bool.and_eq_true] at team ⊢
  obtain ⟨r, ⟨hr, head⟩, ⟨productive, body⟩, superior⟩ := team
  have owned : low r.head := head ▸ hl
  refine ⟨r, ⟨(same r owned).mp hr, head⟩,
    ⟨productive, body_below low r xs ys (closed r hr owned) below body⟩, ?_⟩
  simpa only [Theory.isSuperior, ← priorities] using superior

private theorem attacks_below (low : Literal → Prop) (t u : Theory)
    (closed : ClosedRules low t) (same : SameRules low t u)
    (priorities : t.superiority = u.superiority)
    (complements : ∀ l, low l → low l.complement)
    (lt lu xs ys : List Literal) (lambda : Agree low lt lu) (below : Below low xs ys)
    (l : Literal) (hl : low l)
    (attacks : Closure.allAttacksDefeated t l lt xs = true) :
    Closure.allAttacksDefeated u l lu ys = true := by
  simp only [Closure.allAttacksDefeated, List.all_eq_true, Theory.rulesWithHead,
    List.mem_filter, beq_iff_eq, Bool.or_eq_true, Bool.not_eq_true'] at attacks ⊢
  intro attacker member
  have owned : low attacker.head := member.2 ▸ complements l hl
  have present := (same attacker owned).mpr member.1
  rcases attacks attacker ⟨present, member.2⟩ with (fact | unreachable) | defeated
  · exact Or.inl (Or.inl fact)
  · left; right
    have equal := body_agree low attacker lt lu (closed attacker present owned) lambda
    change attacker.bodySatisfied lu = false
    rw [← equal]
    exact unreachable
  · right
    exact team_below low t u closed same priorities xs ys below l hl attacker defeated

private theorem canProve_below (low : Literal → Prop) (t u : Theory)
    (closed : ClosedRules low t) (same : SameRules low t u)
    (priorities : t.superiority = u.superiority)
    (complements : ∀ l, low l → low l.complement)
    (dt du lt lu xs ys : List Literal) (delta : Agree low dt du) (lambda : Agree low lt lu)
    (below : Below low xs ys) (l : Literal) (hl : low l)
    (can : Closure.canProve t l dt lt xs = true) : Closure.canProve u l du lu ys = true := by
  have hd := contains_agree low dt du delta l hl
  have hc := contains_agree low dt du delta l.complement (complements l hl)
  simp only [Closure.canProve, ← hd, ← hc] at can ⊢
  by_cases opposite : dt.contains l.complement = true
  · simp only [if_pos opposite, Bool.false_eq_true] at can
  · simp only [if_neg opposite] at can ⊢
    by_cases definite : dt.contains l = true
    · simp only [if_pos definite]
    · simp only [if_neg definite, Bool.and_eq_true] at can ⊢
      refine ⟨?_, attacks_below low t u closed same priorities complements lt lu xs ys
        lambda below l hl can.2⟩
      simp only [List.any_eq_true, Theory.rulesWithHead, List.mem_filter, beq_iff_eq,
        Bool.and_eq_true] at can ⊢
      obtain ⟨r, ⟨hr, head⟩, productive, body⟩ := can.1
      have owned : low r.head := head ▸ hl
      exact ⟨r, ⟨(same r owned).mp hr, head⟩, productive,
        body_below low r xs ys (closed r hr owned) below body⟩

private theorem partial_step_below (low : Literal → Prop) (t u : Theory)
    (closed : ClosedRules low t) (same : SameRules low t u)
    (priorities : t.superiority = u.superiority)
    (complements : ∀ l, low l → low l.complement)
    (dt du lt lu xs ys : List Literal) (delta : Agree low dt du) (lambda : Agree low lt lu)
    (below : Below low xs ys) :
    Below low (Closure.partialStep t dt lt xs) (Closure.partialStep u du lu ys) := by
  intro l hl member
  simp only [Closure.partialStep, List.mem_dedup, List.mem_append, List.mem_filter,
    Bool.and_eq_true] at member
  rcases member with old | ⟨candidate, _, can⟩
  · exact Properties.mem_partialStep_of_mem u du lu ys l (below l hl old)
  · exact partial_admits u du lu ys l ((lambda l hl).mp candidate)
      (canProve_below low t u closed same priorities complements dt du lt lu xs ys
        delta lambda below l hl can)


private theorem loop_invariant (step : List Literal → List Literal)
    (go : List Literal → Nat → List Literal)
    (zero : ∀ xs, go xs 0 = xs)
    (succ : ∀ xs n, go xs (n + 1) =
      if (step xs).length == xs.length then xs else go (step xs) n)
    (P : List Literal → Prop) (preserved : ∀ xs, P xs → P (step xs))
    (xs : List Literal) (initial : P xs) (count : Nat) : P (go xs count) := by
  induction count generalizing xs with
  | zero => simpa only [zero] using initial
  | succ count ih =>
    rw [succ]
    split
    · exact initial
    · exact ih (step xs) (preserved xs initial)

private theorem delta_below (low : Literal → Prop) (t u : Theory)
    (closed : ClosedRules low t) (same : SameRules low t u) :
    Below low (Closure.deltaClose t) (Closure.deltaClose u) := by
  apply loop_invariant (Closure.deltaStep t) (Closure.deltaClose.go t)
    (fun _ => rfl) (fun _ _ => rfl) (fun xs => Below low xs (Closure.deltaClose u))
  · intro xs below
    have moved := delta_step_below low t u closed same xs (Closure.deltaClose u) below
    simpa only [Properties.deltaClose_fixedpoint] using moved
  · intro l hl member
    apply Properties.mem_deltaClose_go_of_mem
    simp only [List.mem_dedup, List.mem_map, Theory.facts, List.mem_filter] at member ⊢
    obtain ⟨r, ⟨hr, fact⟩, head⟩ := member
    exact ⟨r, ⟨(same r (head ▸ hl)).mp hr, fact⟩, head⟩

private theorem lambda_below (low : Literal → Prop) (t u : Theory)
    (closed : ClosedRules low t) (same : SameRules low t u)
    (complements : ∀ l, low l → low l.complement)
    (delta : Agree low (Closure.deltaClose t) (Closure.deltaClose u)) :
    Below low (Closure.lambdaClose t (Closure.deltaClose t))
      (Closure.lambdaClose u (Closure.deltaClose u)) := by
  apply loop_invariant (Closure.lambdaStep t (Closure.deltaClose t))
    (Closure.lambdaClose.go t (Closure.deltaClose t))
    (fun _ => rfl) (fun _ _ => rfl)
    (fun xs => Below low xs (Closure.lambdaClose u (Closure.deltaClose u)))
  · intro xs below
    have moved := lambda_step_below low t u closed same complements _ _ xs _ delta below
    simpa only [Properties.lambdaClose_fixedpoint] using moved
  · intro l hl member
    exact Properties.delta_subset_lambda u l ((delta l hl).mp member)

private theorem partial_below (low : Literal → Prop) (t u : Theory)
    (closed : ClosedRules low t) (same : SameRules low t u)
    (priorities : t.superiority = u.superiority)
    (complements : ∀ l, low l → low l.complement)
    (delta : Agree low (Closure.deltaClose t) (Closure.deltaClose u))
    (lambda : Agree low (Closure.lambdaClose t (Closure.deltaClose t))
      (Closure.lambdaClose u (Closure.deltaClose u))) :
    Below low (Closure.partialClose t (Closure.deltaClose t) (Closure.lambdaClose t (Closure.deltaClose t)))
      (Closure.partialClose u (Closure.deltaClose u) (Closure.lambdaClose u (Closure.deltaClose u))) := by
  apply loop_invariant
    (Closure.partialStep t (Closure.deltaClose t) (Closure.lambdaClose t (Closure.deltaClose t)))
    (Closure.partialClose.go t (Closure.deltaClose t) (Closure.lambdaClose t (Closure.deltaClose t)))
    (fun _ => rfl) (fun _ _ => rfl)
    (fun xs => Below low xs
      (Closure.partialClose u (Closure.deltaClose u) (Closure.lambdaClose u (Closure.deltaClose u))))
  · intro xs below
    have moved := partial_step_below low t u closed same priorities complements _ _ _ _ xs _
      delta lambda below
    simpa only [Properties.partialClose_fixedpoint] using moved
  · intro l hl member
    have gated : l ∈ Closure.deltaClose t ∧ l.complement ∉ Closure.deltaClose t := by
      simpa [Closure.gatedDelta] using member
    apply Properties.delta_subset_partial u l ((delta l hl).mp gated.1)
    intro opposite
    exact gated.2 ((delta l.complement (complements l hl)).mpr opposite)


set_option maxHeartbeats 800000 in
/-- Two theories agreeing on a dependency-closed set of domains compute the
same three closures there, even with different finite universes and fuel bounds. -/
theorem reason_agrees_on_closed_domains (low : Literal → Prop) (t u : Theory)
    (closedT : ClosedRules low t) (closedU : ClosedRules low u)
    (same : SameRules low t u) (priorities : t.superiority = u.superiority)
    (complements : ∀ l, low l → low l.complement) :
    Agree low (reason t).delta (reason u).delta ∧
    Agree low (reason t).lambda (reason u).lambda ∧
    Agree low (reason t).partial_ (reason u).partial_ := by
  have reverse : SameRules low u t := fun r hr => (same r hr).symm
  have delta : Agree low (Closure.deltaClose t) (Closure.deltaClose u) := fun l hl =>
    ⟨delta_below low t u closedT same l hl, delta_below low u t closedU reverse l hl⟩
  have deltaRev : Agree low (Closure.deltaClose u) (Closure.deltaClose t) :=
    fun l hl => (delta l hl).symm
  have lambda : Agree low (Closure.lambdaClose t (Closure.deltaClose t))
      (Closure.lambdaClose u (Closure.deltaClose u)) := fun l hl =>
    ⟨lambda_below low t u closedT same complements delta l hl,
     lambda_below low u t closedU reverse complements deltaRev l hl⟩
  refine ⟨delta, lambda, fun l hl => ⟨?_, ?_⟩⟩
  · exact partial_below low t u closedT same priorities complements delta lambda l hl
  · exact partial_below low u t closedU reverse priorities.symm complements deltaRev
      (fun l hl => (lambda l hl).symm) l hl

/-- Scheduled prefix evaluation agrees with full-theory reasoning in every
completed domain, including delta, lambda, and conflict-resolved partial support. -/
theorem groundPrefix_equivalent (owner : Literal → Nat) (theory : Theory) (stage : Nat)
    (ordered : ∀ r ∈ theory.rules, ∀ l ∈ r.body, owner l ≤ owner r.head)
    (complements : ∀ l, owner l.complement = owner l) :
    Agree (fun l => owner l ≤ stage) (reason (groundPrefix owner theory stage)).delta (reason theory).delta ∧
    Agree (fun l => owner l ≤ stage) (reason (groundPrefix owner theory stage)).lambda (reason theory).lambda ∧
    Agree (fun l => owner l ≤ stage) (reason (groundPrefix owner theory stage)).partial_ (reason theory).partial_ := by
  apply reason_agrees_on_closed_domains
  · intro r member head l body
    have present := (List.mem_filter.mp member).1
    exact Nat.le_trans (ordered r present l body) head
  · intro r member head l body
    exact Nat.le_trans (ordered r member l body) head
  · intro r hr
    simp [groundPrefix, hr]
  · rfl
  · intro l hl
    simpa only [complements] using hl


private theorem report_filter_agrees (owner : Literal → Nat) (stage : Nat)
    (literals : List Literal) (a b : ReasonResult)
    (delta : Agree (fun l => owner l ≤ stage) a.delta b.delta)
    (partial_ : Agree (fun l => owner l ≤ stage) a.partial_ b.partial_) :
    (reportConclusions literals a).filter (fun c => decide (owner c.literal = stage)) =
    (reportConclusions literals b).filter (fun c => decide (owner c.literal = stage)) := by
  induction literals with
  | nil => rfl
  | cons l rest ih =>
    simp only [reportConclusions, List.flatMap_cons, List.filter_append] at ih ⊢
    rw [ih]
    congr 1
    by_cases owned : owner l = stage
    · have hd := contains_agree _ a.delta b.delta delta l (by omega)
      have hp := contains_agree _ a.partial_ b.partial_ partial_ l (by omega)
      simp only [reportLiteral, hd, hp]
    · simp only [reportLiteral]
      split_ifs <;> simp [owned]

/-- Exact equality of published tagged batches, including negative tags for
literals that occur only in future rule bodies. -/
theorem reasonedStage_equivalent (owner : Literal → Nat) (theory : Theory) (stage : Nat)
    (ordered : ∀ r ∈ theory.rules, ∀ l ∈ r.body, owner l ≤ owner r.head)
    (complements : ∀ l, owner l.complement = owner l) :
    reasonedStage owner theory stage =
      (reason theory).conclusions.filter (fun c => decide (owner c.literal = stage)) := by
  have agreement := groundPrefix_equivalent owner theory stage ordered complements
  unfold reasonedStage
  rw [report_filter_agrees owner stage theory.allLiterals _ (reason theory) agreement.1 agreement.2.2,
    reason_reports]


private theorem advanceGround (owner : Literal → Nat) (theory : Theory)
    (recognized : groundScheduled owner theory = true) (state : StageState) :
    advanceStage owner (groundBackend owner theory) state =
      .ok ⟨state.next + 1, state.conclusions ++ reasonedStage owner theory state.next⟩ := by
  have owned : (reasonedStage owner theory state.next).all
      (fun c => decide (owner c.literal = state.next)) = true := by
    apply List.all_eq_true.mpr
    intro c member
    exact decide_eq_true ((mem_reasonedStage owner theory state.next c).mp member).2
  simp only [advanceStage, groundBackend, recognized, if_true, owned]

/-- Accumulating scheduled batches preserves exactly the full-theory tags for
the stages visited, in addition to the caller's prior conclusions. -/
theorem runGround_membership (owner : Literal → Nat) (theory : Theory)
    (recognized : groundScheduled owner theory = true)
    (complements : ∀ l, owner l.complement = owner l)
    (count : Nat) (before after : StageState)
    (returned : runStages owner (groundBackend owner theory) count before = .ok after)
    (c : Conclusion) :
    c ∈ after.conclusions ↔ c ∈ before.conclusions ∨
      (c ∈ (reason theory).conclusions ∧ before.next ≤ owner c.literal ∧
        owner c.literal < before.next + count) := by
  induction count generalizing before with
  | zero =>
    cases returned
    simp
  | succ count ih =>
    simp only [runStages, advanceGround owner theory recognized] at returned
    rw [ih _ returned]
    rw [reasonedStage_equivalent owner theory before.next
      ((groundScheduled_iff _ _).mp recognized).2.1 complements]
    simp only [List.mem_append, List.mem_filter, decide_eq_true_eq]
    constructor
    · intro member
      rcases member with (old | ⟨full, same⟩) | ⟨full, lower, upper⟩
      · exact Or.inl old
      · exact Or.inr ⟨full, by omega, by omega⟩
      · exact Or.inr ⟨full, by omega, by omega⟩
    · intro member
      rcases member with old | ⟨full, lower, upper⟩
      · exact Or.inl (Or.inl old)
      · by_cases same : owner c.literal = before.next
        · exact Or.inl (Or.inr ⟨full, same⟩)
        · exact Or.inr ⟨full, by omega, by omega⟩

/-- Complete execution reports all four tags exactly as a full-theory run,
restricted only by the explicitly requested number of stages. -/
theorem executeGround_membership (owner : Literal → Nat) (theory : Theory)
    (recognized : groundScheduled owner theory = true)
    (complements : ∀ l, owner l.complement = owner l)
    (count : Nat) (after : StageState)
    (returned : executeStages owner (groundBackend owner theory) count = .ok after)
    (c : Conclusion) : c ∈ after.conclusions ↔
      c ∈ (reason theory).conclusions ∧ owner c.literal < count := by
  simpa using runGround_membership owner theory recognized complements count ⟨0, []⟩ after returned c

end Spindle.Aggregation
