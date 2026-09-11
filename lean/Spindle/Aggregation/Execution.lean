import Spindle.Aggregation.Snapshot

/-!
# Checked execution across stratum boundaries

This is the finite stage driver, parameterized by a local reasoning backend.
The driver checks output ownership and preserves conclusion tags verbatim.
A successful backend call must separately be shown to finish local reasoning;
this protocol does not certify an arbitrary callback as a fixed-point solver.

A fold observes positive conclusions in a frozen prefix. Its value is relative
to that snapshot, not a new SDL +D or +d judgment. In particular, positive +D
proofs for every included row do not prove that the relation cannot grow.
-/
namespace Spindle.Aggregation

structure StageState where
  next : Nat
  conclusions : List Conclusion

def StageState.Valid (owner : Literal → Nat) (state : StageState) : Prop :=
  ∀ c ∈ state.conclusions, owner c.literal < state.next

inductive ExecutionError where
  | backend (message : String)
  | wrongOwner (stage : Nat)
  deriving DecidableEq, Repr

/-- The backend sees the next stage and the prior tagged conclusions. It returns
only this stage's final conclusions, or an explicit error (including limits). -/
abbrev StageBackend := Nat → List Conclusion → Except String (List Conclusion)

/-- Commit a batch only after checking every conclusion's ownership. -/
def advanceStage (owner : Literal → Nat) (backend : StageBackend)
    (state : StageState) : Except ExecutionError StageState :=
  match backend state.next state.conclusions with
  | .error message => .error (.backend message)
  | .ok batch =>
    if batch.all (fun c => decide (owner c.literal = state.next)) then
      .ok ⟨state.next + 1, state.conclusions ++ batch⟩
    else .error (.wrongOwner state.next)

theorem advanceStage_commits (owner : Literal → Nat) (backend : StageBackend)
    (before after : StageState) (returned : advanceStage owner backend before = .ok after) :
    ∃ batch, backend before.next before.conclusions = .ok batch ∧
      (∀ c ∈ batch, owner c.literal = before.next) ∧
      after = ⟨before.next + 1, before.conclusions ++ batch⟩ := by
  unfold advanceStage at returned
  cases called : backend before.next before.conclusions with
  | error message => simp [called] at returned
  | ok batch =>
    simp only [called] at returned
    split at returned
    next owned =>
      refine ⟨batch, rfl, ?_, ?_⟩
      · simpa only [List.all_eq_true, decide_eq_true_eq] using owned
      · simpa only [Except.ok.injEq] using returned.symm
    · simp at returned

theorem advanceStage_valid (owner : Literal → Nat) (backend : StageBackend)
    (before after : StageState) (valid : before.Valid owner)
    (returned : advanceStage owner backend before = .ok after) : after.Valid owner := by
  obtain ⟨batch, _, owned, rfl⟩ := advanceStage_commits owner backend before after returned
  intro c member
  rcases List.mem_append.mp member with old | fresh
  · have bound := valid c old
    dsimp
    omega
  · have equal := owned c fresh
    dsimp
    omega

/-- Execute exactly the supplied number of strata. This bound counts stages;
it is not a fuel bound for the backend's local fixed-point computation. -/
def runStages (owner : Literal → Nat) (backend : StageBackend) :
    Nat → StageState → Except ExecutionError StageState
  | 0, state => .ok state
  | count + 1, state =>
    match advanceStage owner backend state with
    | .error error => .error error
    | .ok next => runStages owner backend count next

/-- Successful execution appends only conclusions owned by the executed stages.
Existing tagged conclusions are retained exactly, without reconstruction. -/
theorem runStages_extends (owner : Literal → Nat) (backend : StageBackend)
    (count : Nat) (before after : StageState)
    (returned : runStages owner backend count before = .ok after) :
    after.next = before.next + count ∧
      ∃ future, after.conclusions = before.conclusions ++ future ∧
        ∀ c ∈ future, before.next ≤ owner c.literal ∧ owner c.literal < after.next := by
  induction count generalizing before with
  | zero =>
    simp only [runStages, Except.ok.injEq] at returned
    subst after
    exact ⟨by omega, [], by simp, by simp⟩
  | succ count ih =>
    simp only [runStages] at returned
    cases stepped : advanceStage owner backend before with
    | error error => simp [stepped] at returned
    | ok next =>
      simp only [stepped] at returned
      obtain ⟨batch, _, owned, equal⟩ := advanceStage_commits owner backend before next stepped
      obtain ⟨stages, future, appended, bounds⟩ := ih next returned
      subst next
      refine ⟨by dsimp at stages; omega, batch ++ future, ?_, ?_⟩
      · simpa only [List.append_assoc] using appended
      · intro c member
        rcases List.mem_append.mp member with fresh | later
        · have stage := owned c fresh
          dsimp at stages
          constructor <;> omega
        · have bound := bounds c later
          dsimp at bound
          constructor <;> omega

theorem runStages_valid (owner : Literal → Nat) (backend : StageBackend)
    (count : Nat) (before after : StageState) (valid : before.Valid owner)
    (returned : runStages owner backend count before = .ok after) : after.Valid owner := by
  obtain ⟨stages, future, appended, bounds⟩ :=
    runStages_extends owner backend count before after returned
  intro c member
  rw [appended] at member
  rcases List.mem_append.mp member with old | fresh
  · have bound := valid c old
    omega
  · exact (bounds c fresh).2

/-- Public entry point starts with no committed conclusions at stage zero. -/
def executeStages (owner : Literal → Nat) (backend : StageBackend) (count : Nat) :
    Except ExecutionError StageState :=
  runStages owner backend count ⟨0, []⟩

theorem executeStages_valid (owner : Literal → Nat) (backend : StageBackend)
    (count : Nat) (after : StageState)
    (returned : executeStages owner backend count = .ok after) :
    after.next = count ∧ after.Valid owner := by
  constructor
  · have stages := (runStages_extends owner backend count ⟨0, []⟩ after returned).1
    simpa using stages
  · exact runStages_valid owner backend count ⟨0, []⟩ after (by simp [StageState.Valid]) returned

/-- Later stages neither remove old evidence nor add a different proof tag for
an earlier-owned literal. Membership refers to the complete tagged conclusion. -/
theorem runStages_old_conclusion_iff (owner : Literal → Nat) (backend : StageBackend)
    (count : Nat) (before after : StageState)
    (returned : runStages owner backend count before = .ok after)
    (c : Conclusion) (earlier : owner c.literal < before.next) :
    c ∈ after.conclusions ↔ c ∈ before.conclusions := by
  obtain ⟨_, future, appended, bounds⟩ := runStages_extends owner backend count before after returned
  rw [appended, List.mem_append]
  constructor
  · intro member
    rcases member with old | fresh
    · exact old
    · have lower := (bounds c fresh).1
      omega
  · exact Or.inl

/-- Snapshot-scoped aggregate observation; this function emits no SDL proof tag. -/
def foldAt {α : Type} (owner : Literal → Nat) (consumer : Nat)
    (conclusions : List Conclusion) (reducer : Reducer α) (seed : Option α)
    (select : Literal → Bool) (extract : Literal → α) : Option α :=
  reducer.eval seed (values
    (snapshot (frozenRows (fun c => c.literal) owner consumer conclusions)) select extract)

/-- Once the driver has reached a consumer boundary, subsequent successful
stages cannot change any fold over that boundary's frozen input. -/
theorem runStages_preserves_fold {α : Type} (owner : Literal → Nat) (backend : StageBackend)
    (count : Nat) (before after : StageState) (consumer : Nat)
    (reached : consumer ≤ before.next)
    (returned : runStages owner backend count before = .ok after)
    (reducer : Reducer α) (seed : Option α)
    (select : Literal → Bool) (extract : Literal → α) :
    foldAt owner consumer after.conclusions reducer seed select extract =
      foldAt owner consumer before.conclusions reducer seed select extract := by
  obtain ⟨_, future, appended, bounds⟩ := runStages_extends owner backend count before after returned
  unfold foldAt
  rw [appended, frozenRows_append_future]
  intro c member
  have lower := (bounds c member).1
  omega

end Spindle.Aggregation
