import Spindle.Modal.Basic

namespace Spindle.Modal

abbrev State := List (ConclusionType × Lit)
abbrev tags := Spindle.Aggregation.Operational.tags

/-- Close each mentioned modal atom/window under inner and outer polarity and
O/P modes. Plain literals retain ordinary complement closure. -/
def variants (l : Lit) : List Lit :=
  if l.mode = .plain then [l, { l with inner := !l.inner }]
  else [.obligation, .permission].flatMap fun mode =>
    [false, true].flatMap fun inner =>
      [false, true].map fun outer => { l with mode, inner, outer }

def literals (t : MTheory) : List Lit :=
  ((t.rules.flatMap fun r => r.head :: r.body).flatMap variants).dedup

def univ (t : MTheory) : State :=
  ((literals t).flatMap fun l => tags.map (·, l)).dedup

def has (s : State) (tag : ConclusionType) (l : Lit) : Bool := s.contains (tag, l)

def positivePremise (t : MTheory) (s : State) (tag : ConclusionType) (l : Lit) : Bool :=
  (literals t).any fun m => premiseMatches l m && has s tag m

def negativePremise (t : MTheory) (s : State) (tag : ConclusionType) (l : Lit) : Bool :=
  ((literals t).filter (premiseMatches l)).all (has s tag)

def applicable (t : MTheory) (s : State) (tag : ConclusionType) (r : MRule) : Bool :=
  r.body.all fun l => positivePremise t s tag l ||
    (decide (tag = .defeasiblyProvable) && l.negativeModal &&
      negativePremise t s .defeasiblyNotProvable l.negate)

def discarded (t : MTheory) (s : State) (tag : ConclusionType) (r : MRule) : Bool :=
  r.body.any fun l =>
    if tag = .defeasiblyNotProvable ∧ l.negativeModal = true then
      positivePremise t s .defeasiblyProvable l.negate
    else negativePremise t s tag l

def heads (t : MTheory) (l : Lit) : List MRule := t.rules.filter (fun r => decide (r.head = l))
def attackers (t : MTheory) (l : Lit) : List MRule := t.rules.filter (fun r => conflict l r.head)
def defenders (t : MTheory) (l : Lit) (a : MRule) : List MRule := t.rules.filter (defends l a)

/-- Four constructive inference conditions, independent of the Rust worklist. -/
def infer (t : MTheory) (s : State) (tag : ConclusionType) (l : Lit) : Bool :=
  match tag with
  | .definitelyProvable => (heads t l).any fun r =>
      decide (r.kind = .fact) || (decide (r.kind = .strict) && applicable t s .definitelyProvable r)
  | .definitelyNotProvable => (heads t l).all fun r =>
      decide (r.kind ≠ .fact) && (decide (r.kind ≠ .strict) || discarded t s .definitelyNotProvable r)
  | .defeasiblyProvable => has s .definitelyProvable l ||
      (((literals t).filter (conflict l)).all (has s .definitelyNotProvable) &&
       ((heads t l).any fun r => r.productive && applicable t s .defeasiblyProvable r) &&
       (attackers t l).all fun a => discarded t s .defeasiblyNotProvable a ||
         (defenders t l a).any fun d => applicable t s .defeasiblyProvable d && t.superior d.label a.label)
  | .defeasiblyNotProvable => has s .definitelyNotProvable l &&
      (((literals t).any fun m => conflict l m && has s .definitelyProvable m) ||
       ((heads t l).all fun r => !r.productive || discarded t s .defeasiblyNotProvable r) ||
       ((attackers t l).any fun a => applicable t s .defeasiblyProvable a &&
         (defenders t l a).all fun d => !t.superior d.label a.label || discarded t s .defeasiblyNotProvable d))

def step (t : MTheory) (s : State) : State :=
  s ++ (univ t).filter (fun pair => !s.contains pair && infer t s pair.1 pair.2)

def close (t : MTheory) : State :=
  Spindle.Aggregation.FiniteIteration.close (step t) [] ((univ t).length + 1)

def reason (t : MTheory) : State := close t.normalize

theorem step_grows (t : MTheory) (s : State) : ∃ added, step t s = s ++ added := ⟨_, rfl⟩

theorem step_preserves (t : MTheory) (s : State) (hs : s.Nodup) (sub : s ⊆ univ t) :
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

theorem close_fixedpoint (t : MTheory) : step t (close t) = close t := by
  apply Spindle.Aggregation.FiniteIteration.fixed _ (univ t) (step_grows t) (step_preserves t)
  · exact List.nodup_nil
  · exact List.nil_subset _
  · simp

/-- Every enabled rule of inference over the finite domain is saturated. -/
theorem close_complete (t : MTheory) (tag : ConclusionType) (l : Lit)
    (domain : (tag, l) ∈ univ t) (enabled : infer t (close t) tag l = true) :
    (tag, l) ∈ close t := by
  by_cases member : (tag, l) ∈ close t
  · exact member
  · have fresh : (tag, l) ∈ step t (close t) := by simp [step, domain, member, enabled]
    rwa [close_fixedpoint] at fresh

end Spindle.Modal
