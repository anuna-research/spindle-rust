/-
  Spindle Temporal Interval

  Defines Temporal (interval) as a pair (start: TimePoint, end: TimePoint) with
  start ≤ end invariant. Provides intersection, containment, and adjacency
  predicates. Proves that interval well-formedness is preserved by intersection.
-/
import Spindle.Arith.TimePoint

namespace Spindle.Arith

/-! ## Interval type -/

/-- A temporal interval with start ≤ end invariant. -/
structure Interval where
  start : TimePoint
  stop  : TimePoint
  valid : start ≤ stop
  deriving Repr

/-! ## Smart constructor -/

/-- Attempt to build an interval; returns `none` if start > end. -/
def Interval.mk? (s e : TimePoint) : Option Interval :=
  if h : s ≤ e then some ⟨s, e, h⟩ else none

/-- Build an interval from equal endpoints (always valid). -/
def Interval.point (t : TimePoint) : Interval :=
  ⟨t, t, TimePoint.le_refl t⟩

/-- The full timeline from -∞ to +∞. -/
def Interval.full : Interval :=
  ⟨.negInf, .posInf, TimePoint.negInf_le _⟩

/-! ## Containment -/

/-- A timepoint is contained in an interval. -/
def Interval.contains (i : Interval) (t : TimePoint) : Prop :=
  i.start ≤ t ∧ t ≤ i.stop

/-- One interval is a subset of another. -/
def Interval.subset (a b : Interval) : Prop :=
  b.start ≤ a.start ∧ a.stop ≤ b.stop

instance (i : Interval) (t : TimePoint) : Decidable (i.contains t) :=
  inferInstanceAs (Decidable (_ ∧ _))

instance (a b : Interval) : Decidable (a.subset b) :=
  inferInstanceAs (Decidable (_ ∧ _))

/-! ## Adjacency -/

/-- Two intervals are adjacent if one ends exactly where the other begins
    (with no gap and no overlap). -/
def Interval.adjacent (a b : Interval) : Prop :=
  a.stop = b.start ∨ b.stop = a.start

instance (a b : Interval) : Decidable (a.adjacent b) :=
  inferInstanceAs (Decidable (_ ∨ _))

/-! ## Overlap -/

/-- Two intervals overlap iff their intersection is non-empty:
    a.start ≤ b.stop ∧ b.start ≤ a.stop. -/
def Interval.overlaps (a b : Interval) : Prop :=
  a.start ≤ b.stop ∧ b.start ≤ a.stop

instance (a b : Interval) : Decidable (a.overlaps b) :=
  inferInstanceAs (Decidable (_ ∧ _))

/-! ## Max / Min helpers -/

/-- Maximum of two timepoints. -/
def TimePoint.max (a b : TimePoint) : TimePoint :=
  if a ≤ b then b else a

/-- Minimum of two timepoints. -/
def TimePoint.min (a b : TimePoint) : TimePoint :=
  if a ≤ b then a else b

theorem TimePoint.le_max_left (a b : TimePoint) : a ≤ TimePoint.max a b := by
  simp only [TimePoint.max]
  split
  · assumption
  · exact TimePoint.le_refl a

theorem TimePoint.le_max_right (a b : TimePoint) : b ≤ TimePoint.max a b := by
  simp only [TimePoint.max]
  split
  · exact TimePoint.le_refl b
  · next h =>
    rcases TimePoint.le_total a b with hab | hba
    · exact absurd hab h
    · exact hba

theorem TimePoint.min_le_left (a b : TimePoint) : TimePoint.min a b ≤ a := by
  simp only [TimePoint.min]
  split
  · exact TimePoint.le_refl a
  · next h =>
    rcases TimePoint.le_total a b with hab | hba
    · exact absurd hab h
    · exact hba

theorem TimePoint.min_le_right (a b : TimePoint) : TimePoint.min a b ≤ b := by
  simp only [TimePoint.min]
  split
  · assumption
  · exact TimePoint.le_refl b

theorem TimePoint.max_comm (a b : TimePoint) : TimePoint.max a b = TimePoint.max b a := by
  simp only [TimePoint.max]
  split <;> split
  · next hab hba => exact TimePoint.le_antisymm hba hab
  · rfl
  · rfl
  · next hab hba =>
    rcases TimePoint.le_total a b with h | h
    · exact absurd h hab
    · exact absurd h hba

theorem TimePoint.min_comm (a b : TimePoint) : TimePoint.min a b = TimePoint.min b a := by
  simp only [TimePoint.min]
  split <;> split
  · next hab hba => exact TimePoint.le_antisymm hab hba
  · rfl
  · rfl
  · next hab hba =>
    rcases TimePoint.le_total a b with h | h
    · exact absurd h hab
    · exact absurd h hba

/-! ## Intersection -/

/-- The intersection of two overlapping intervals. Returns `none` if they don't overlap. -/
def Interval.intersect (a b : Interval) : Option Interval :=
  let s := TimePoint.max a.start b.start
  let e := TimePoint.min a.stop b.stop
  if h : s ≤ e then some ⟨s, e, h⟩ else none

/-! ## Containment properties -/

/-- Every interval is a subset of itself. -/
theorem Interval.subset_refl (i : Interval) : i.subset i :=
  ⟨TimePoint.le_refl i.start, TimePoint.le_refl i.stop⟩

/-- Subset is transitive. -/
theorem Interval.subset_trans {a b c : Interval}
    (hab : a.subset b) (hbc : b.subset c) : a.subset c :=
  ⟨TimePoint.le_trans hbc.1 hab.1, TimePoint.le_trans hab.2 hbc.2⟩

/-- A point in a sub-interval is in the super-interval. -/
theorem Interval.contains_of_subset {a b : Interval} {t : TimePoint}
    (hsub : a.subset b) (hcon : a.contains t) : b.contains t :=
  ⟨TimePoint.le_trans hsub.1 hcon.1, TimePoint.le_trans hcon.2 hsub.2⟩

/-! ## Main theorem: intersection preserves well-formedness -/

/-- Helper: max of two starts ≤ min of two stops when intervals overlap. -/
private theorem max_le_min_of_overlaps {a b : Interval} (h : a.overlaps b) :
    TimePoint.max a.start b.start ≤ TimePoint.min a.stop b.stop := by
  simp only [TimePoint.max, TimePoint.min]
  -- case split on (a.start ≤ b.start) then (a.stop ≤ b.stop)
  split <;> split
  · -- a.start ≤ b.start, a.stop ≤ b.stop → goal: b.start ≤ a.stop
    exact h.2
  · -- a.start ≤ b.start, ¬(a.stop ≤ b.stop) → goal: b.start ≤ b.stop
    exact b.valid
  · -- ¬(a.start ≤ b.start), a.stop ≤ b.stop → goal: a.start ≤ a.stop
    exact a.valid
  · -- ¬(a.start ≤ b.start), ¬(a.stop ≤ b.stop) → goal: a.start ≤ b.stop
    exact h.1

/-- **Main theorem**: if two intervals overlap, their intersection is well-formed.
    That is, `intersect` returns `some` and the resulting interval satisfies
    start ≤ stop. This follows directly from the definition since
    we check `s ≤ e` in the constructor. -/
theorem Interval.intersect_isSome_of_overlaps {a b : Interval} (h : a.overlaps b) :
    (a.intersect b).isSome = true := by
  simp only [Interval.intersect]
  have hle := max_le_min_of_overlaps h
  simp [hle]

/-- The intersection of overlapping intervals is a subset of the first interval. -/
theorem Interval.intersect_subset_left {a b : Interval}
    (hi : Interval) (heq : a.intersect b = some hi) : hi.subset a := by
  simp only [Interval.intersect] at heq
  split at heq
  · injection heq with heq
    rw [← heq]
    exact ⟨TimePoint.le_max_left a.start b.start, TimePoint.min_le_left a.stop b.stop⟩
  · exact absurd heq (by simp)

/-- The intersection of overlapping intervals is a subset of the second interval. -/
theorem Interval.intersect_subset_right {a b : Interval}
    (hi : Interval) (heq : a.intersect b = some hi) : hi.subset b := by
  simp only [Interval.intersect] at heq
  split at heq
  · injection heq with heq
    rw [← heq]
    exact ⟨TimePoint.le_max_right a.start b.start, TimePoint.min_le_right a.stop b.stop⟩
  · exact absurd heq (by simp)

end Spindle.Arith
