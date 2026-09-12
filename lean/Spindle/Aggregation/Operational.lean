import Spindle.Aggregation.FiniteIteration

/-! Traditional ambiguity-blocking DL(partial), with four constructive tags.
The only implicit negative proofs are for literals with no rules at all;
these satisfy the empty universal premises of -D and then -d. This permits
queries about body-only literals without making missing positive proofs false.
-/
namespace Spindle.Aggregation.Operational

abbrev State := List (ConclusionType × Literal)
def tags : List ConclusionType :=
  [.definitelyProvable, .definitelyNotProvable, .defeasiblyProvable, .defeasiblyNotProvable]

def negative : ConclusionType → Bool
  | .definitelyNotProvable | .defeasiblyNotProvable => true
  | _ => false

def holds (t : Theory) (s : State) (tag : ConclusionType) (l : Literal) : Bool :=
  s.contains (tag, l) || (negative tag && (t.rulesWithHead l).isEmpty)

def bodySat (t : Theory) (s : State) (tag : ConclusionType) (r : Rule) : Bool :=
  r.body.all (holds t s tag)
def discarded (t : Theory) (s : State) (tag : ConclusionType) (r : Rule) : Bool :=
  r.body.any (holds t s tag)

def infer (t : Theory) (s : State) (tag : ConclusionType) (l : Literal) : Bool :=
  match tag with
  | .definitelyProvable => (t.rulesWithHead l).any fun r =>
      r.isFact || ((r.ruleType == .strict) && bodySat t s .definitelyProvable r)
  | .definitelyNotProvable => (t.rulesWithHead l).all fun r =>
      !r.isFact && (!(r.ruleType == .strict) || discarded t s .definitelyNotProvable r)
  | .defeasiblyProvable => holds t s .definitelyProvable l ||
      (holds t s .definitelyNotProvable l.complement &&
       ((t.rulesWithHead l).any fun r => r.isProductive && bodySat t s .defeasiblyProvable r) &&
       (t.rulesWithHead l.complement).all fun a =>
         a.isFact || discarded t s .defeasiblyNotProvable a ||
         (t.rulesWithHead l).any fun d =>
           d.isProductive && bodySat t s .defeasiblyProvable d && t.isSuperior d.label a.label)
  | .defeasiblyNotProvable => holds t s .definitelyNotProvable l &&
      (holds t s .definitelyProvable l.complement ||
       ((t.rulesWithHead l).all fun r => !r.isProductive || discarded t s .defeasiblyNotProvable r) ||
       ((t.rulesWithHead l.complement).any fun a => !a.isFact && bodySat t s .defeasiblyProvable a &&
         (t.rulesWithHead l).all fun d =>
           !d.isProductive || !t.isSuperior d.label a.label || discarded t s .defeasiblyNotProvable d))

/-- Only rule heads need stored proofs; absent heads have immediate negative proofs. -/
def univ (t : Theory) : State :=
  (t.rules.flatMap fun r => tags.map (·, r.head)).dedup

def step (t : Theory) (s : State) : State :=
  s ++ (univ t).filter (fun pair => !s.contains pair && infer t s pair.1 pair.2)

def close (t : Theory) : State :=
  FiniteIteration.close (step t) [] ((univ t).length + 1)

structure Result where
  theory : Theory
  state : State

def Result.has (r : Result) (tag : ConclusionType) (l : Literal) : Bool :=
  holds r.theory r.state tag l

def Result.report (r : Result) (l : Literal) : List Conclusion :=
  tags.filterMap fun tag => if r.has tag l then some ⟨l, tag⟩ else none

def Result.conclusions (r : Result) : List Conclusion :=
  r.theory.allLiterals.flatMap r.report

def reason (t : Theory) : Result := ⟨t, close t⟩

theorem step_grows (t : Theory) (s : State) : ∃ added, step t s = s ++ added :=
  ⟨_, rfl⟩

theorem step_preserves (t : Theory) (s : State) (hs : s.Nodup) (sub : s ⊆ univ t) :
    (step t s).Nodup ∧ step t s ⊆ univ t := by
  constructor
  · rw [step, List.nodup_append]
    refine ⟨hs, (List.nodup_dedup _).filter _, ?_⟩
    intro x hx y hy eq
    subst y
    have absent := (List.mem_filter.mp hy).2
    simp at absent
    exact absent.1 hx
  · intro x hx
    rcases List.mem_append.mp hx with old | fresh
    · exact sub old
    · exact (List.mem_filter.mp fresh).1

theorem close_fixedpoint (t : Theory) : step t (close t) = close t := by
  apply FiniteIteration.fixed _ (univ t) (step_grows t) (step_preserves t)
  · exact List.nodup_nil
  · exact List.nil_subset _
  · simp

end Spindle.Aggregation.Operational
