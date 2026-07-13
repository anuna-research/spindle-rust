/-
  Allen Interval Relations

  Defines all 13 Allen interval relations and proves:
  1. JEPD: exactly one relation holds between any two proper intervals
  2. Inverse is an involution: inverse(inverse(r)) = r
  3. Equals is reflexive
-/
import Spindle.Arith.Interval

namespace Spindle.Arith

/-! ## Allen relation type -/

/-- The 13 Allen interval relations. -/
inductive AllenRelation where
  | before | after
  | meets | metBy
  | overlaps | overlappedBy
  | starts | startedBy
  | during | contains
  | finishes | finishedBy
  | equals
  deriving Repr, BEq, DecidableEq

/-! ## Inverse -/

/-- The converse (inverse) of an Allen relation. -/
def AllenRelation.inverse : AllenRelation → AllenRelation
  | .before       => .after
  | .after        => .before
  | .meets        => .metBy
  | .metBy        => .meets
  | .overlaps     => .overlappedBy
  | .overlappedBy => .overlaps
  | .starts       => .startedBy
  | .startedBy    => .starts
  | .during       => .contains
  | .contains     => .during
  | .finishes     => .finishedBy
  | .finishedBy   => .finishes
  | .equals       => .equals

/-- Inverse is an involution. -/
theorem AllenRelation.inverse_involution (r : AllenRelation) :
    r.inverse.inverse = r := by
  cases r <;> rfl

/-! ## Holds predicate -/

/-- When an Allen relation holds between two intervals. -/
def AllenRelation.holds (r : AllenRelation) (x y : Interval) : Prop :=
  match r with
  | .before       => x.stop < y.start
  | .after        => y.stop < x.start
  | .meets        => x.stop = y.start
  | .metBy        => y.stop = x.start
  | .overlaps     => x.start < y.start ∧ y.start < x.stop ∧ x.stop < y.stop
  | .overlappedBy => y.start < x.start ∧ x.start < y.stop ∧ y.stop < x.stop
  | .starts       => x.start = y.start ∧ x.stop < y.stop
  | .startedBy    => x.start = y.start ∧ y.stop < x.stop
  | .during       => y.start < x.start ∧ x.stop < y.stop
  | .contains     => x.start < y.start ∧ y.stop < x.stop
  | .finishes     => x.stop = y.stop ∧ y.start < x.start
  | .finishedBy   => x.stop = y.stop ∧ x.start < y.start
  | .equals       => x.start = y.start ∧ x.stop = y.stop

/-- Equals is reflexive. -/
theorem AllenRelation.equals_refl (x : Interval) :
    AllenRelation.equals.holds x x :=
  ⟨rfl, rfl⟩

/-- If r holds for (x, y) then r.inverse holds for (y, x). -/
theorem AllenRelation.inverse_holds {r : AllenRelation} {x y : Interval}
    (h : r.holds x y) : r.inverse.holds y x := by
  cases r
  · exact h                         -- before → after
  · exact h                         -- after → before
  · exact h                         -- meets → metBy
  · exact h                         -- metBy → meets
  · exact h                         -- overlaps → overlappedBy
  · exact h                         -- overlappedBy → overlaps
  · exact ⟨h.1.symm, h.2⟩          -- starts → startedBy
  · exact ⟨h.1.symm, h.2⟩          -- startedBy → starts
  · exact h                         -- during → contains
  · exact h                         -- contains → during
  · exact ⟨h.1.symm, h.2⟩          -- finishes → finishedBy
  · exact ⟨h.1.symm, h.2⟩          -- finishedBy → finishes
  · exact ⟨h.1.symm, h.2.symm⟩     -- equals → equals

/-! ## Proper intervals -/

/-- A proper (non-degenerate) interval has start strictly less than stop. -/
def Interval.proper (i : Interval) : Prop := i.start < i.stop

instance (i : Interval) : Decidable i.proper :=
  inferInstanceAs (Decidable (i.start < i.stop))

/-! ## Auxiliary ordering lemmas -/

private theorem TimePoint.lt_of_le_of_lt' {a b c : TimePoint}
    (h1 : a ≤ b) (h2 : b < c) : a < c := by
  show TimePoint.lt a c
  have h1' : a = b ∨ TimePoint.lt a b := h1
  have h2' : TimePoint.lt b c := h2
  rcases h1' with rfl | h1'
  · exact h2'
  · exact TimePoint.lt_trans h1' h2'

private theorem TimePoint.lt_of_lt_of_le' {a b c : TimePoint}
    (h1 : a < b) (h2 : b ≤ c) : a < c := by
  show TimePoint.lt a c
  have h1' : TimePoint.lt a b := h1
  have h2' : b = c ∨ TimePoint.lt b c := h2
  rcases h2' with rfl | h2'
  · exact h1'
  · exact TimePoint.lt_trans h1' h2'

private theorem TimePoint.gt_of_not_lt_of_ne {a b : TimePoint}
    (h1 : ¬(a < b)) (h2 : a ≠ b) : b < a := by
  rcases TimePoint.lt_trichotomy a b with h | rfl | h
  · exact absurd h h1
  · exact absurd rfl h2
  · exact h

/-! ## Classification -/

/-- Classify the Allen relation between two intervals. -/
def AllenRelation.classify (x y : Interval) : AllenRelation :=
  if x.start < y.start then
    if x.stop < y.start then .before
    else if x.stop = y.start then .meets
    else if x.stop < y.stop then .overlaps
    else if x.stop = y.stop then .finishedBy
    else .contains
  else if x.start = y.start then
    if x.stop < y.stop then .starts
    else if x.stop = y.stop then .equals
    else .startedBy
  else
    if y.stop < x.start then .after
    else if y.stop = x.start then .metBy
    else if x.stop < y.stop then .during
    else if x.stop = y.stop then .finishes
    else .overlappedBy

/-! ## JEPD: Jointly Exhaustive -/

/-- Classification always produces a valid relation (exhaustive). -/
theorem AllenRelation.classify_holds (x y : Interval) :
    (classify x y).holds x y := by
  unfold classify
  split
  next hss =>
    split
    next h => exact h
    next h =>
      split
      next heq => exact heq
      next hneq =>
        have hgt := TimePoint.gt_of_not_lt_of_ne h hneq
        split
        next hee => exact ⟨hss, hgt, hee⟩
        next hee =>
          split
          next heqe => exact ⟨heqe, hss⟩
          next hneqe => exact ⟨hss, TimePoint.gt_of_not_lt_of_ne hee hneqe⟩
  next hss =>
    split
    next heqs =>
      split
      next hee => exact ⟨heqs, hee⟩
      next hee =>
        split
        next heqe => exact ⟨heqs, heqe⟩
        next hneqe => exact ⟨heqs, TimePoint.gt_of_not_lt_of_ne hee hneqe⟩
    next hneqs =>
      have hgt_ss := TimePoint.gt_of_not_lt_of_ne hss hneqs
      split
      next h => exact h
      next h =>
        split
        next heq => exact heq
        next hneq =>
          have hgt_es := TimePoint.gt_of_not_lt_of_ne h hneq
          split
          next hee => exact ⟨hgt_ss, hee⟩
          next hee =>
            split
            next heqe => exact ⟨heqe, hgt_ss⟩
            next hneqe => exact ⟨hgt_ss, hgt_es, TimePoint.gt_of_not_lt_of_ne hee hneqe⟩

/-! ## JEPD: Pairwise Disjoint -/

/-- At most one relation holds between proper intervals (unique). -/
theorem AllenRelation.holds_unique {x y : Interval}
    (hx : x.proper) (hy : y.proper) {r : AllenRelation}
    (h : r.holds x y) : r = classify x y := by
  unfold Interval.proper at hx hy
  cases r <;> simp only [holds] at h <;> unfold classify
  -- For each Allen relation case, derive all comparison facts needed
  -- to disambiguate the classify if-tree, then use split to resolve.
  -- before: h : x.stop < y.start
  · have : x.start < y.start := TimePoint.lt_of_le_of_lt' x.valid h
    split <;> first | rfl | contradiction
  -- after: h : y.stop < x.start
  · have hgt : y.start < x.start := TimePoint.lt_of_le_of_lt' y.valid h
    have : ¬(x.start < y.start) := fun hc => TimePoint.lt_antisymm hc hgt
    have : ¬(x.start = y.start) := fun heq => by
      rw [heq] at hgt; exact TimePoint.lt_irrefl _ hgt
    split <;> first | rfl | contradiction
  -- meets: h : x.stop = y.start
  · have : x.start < y.start := h ▸ hx
    have : ¬(x.stop < y.start) := by rw [h]; exact fun hc => TimePoint.lt_irrefl _ hc
    split <;> first | rfl | contradiction
  -- metBy: h : y.stop = x.start
  · have hgt : y.start < x.start := h ▸ hy
    have : ¬(x.start < y.start) := fun hc => TimePoint.lt_antisymm hc hgt
    have : ¬(x.start = y.start) := fun heq => by
      rw [heq] at hgt; exact TimePoint.lt_irrefl _ hgt
    have : ¬(y.stop < x.start) := by rw [h]; exact fun hc => TimePoint.lt_irrefl _ hc
    split <;> first | rfl | contradiction
  -- overlaps: x.start < y.start ∧ y.start < x.stop ∧ x.stop < y.stop
  · obtain ⟨hss, hys_xe, hee⟩ := h
    have : ¬(x.stop < y.start) := fun hc => TimePoint.lt_antisymm hys_xe hc
    have : ¬(x.stop = y.start) := fun heq => by rw [heq] at hys_xe; exact TimePoint.lt_irrefl _ hys_xe
    split <;> first | rfl | contradiction
  -- overlappedBy: y.start < x.start ∧ x.start < y.stop ∧ y.stop < x.stop
  · obtain ⟨hss, hxs_ye, hee⟩ := h
    have : ¬(x.start < y.start) := fun hc => TimePoint.lt_antisymm hc hss
    have : ¬(x.start = y.start) := fun heq => by rw [← heq] at hss; exact TimePoint.lt_irrefl _ hss
    have : ¬(y.stop < x.start) := fun hc => TimePoint.lt_antisymm hxs_ye hc
    have : ¬(y.stop = x.start) := fun heq => by rw [heq] at hxs_ye; exact TimePoint.lt_irrefl _ hxs_ye
    have : ¬(x.stop < y.stop) := fun hc => TimePoint.lt_antisymm hc hee
    have : ¬(x.stop = y.stop) := fun heq => by rw [heq] at hee; exact TimePoint.lt_irrefl _ hee
    split <;> first | rfl | contradiction
  -- starts: x.start = y.start ∧ x.stop < y.stop
  · obtain ⟨heqs, hee⟩ := h
    have : ¬(x.start < y.start) := by rw [heqs]; exact fun hc => TimePoint.lt_irrefl _ hc
    split <;> first | rfl | contradiction
  -- startedBy: x.start = y.start ∧ y.stop < x.stop
  · obtain ⟨heqs, hee⟩ := h
    have : ¬(x.start < y.start) := by rw [heqs]; exact fun hc => TimePoint.lt_irrefl _ hc
    have : ¬(x.stop < y.stop) := fun hc => TimePoint.lt_antisymm hc hee
    have : ¬(x.stop = y.stop) := fun heq => by rw [heq] at hee; exact TimePoint.lt_irrefl _ hee
    split <;> first | rfl | contradiction
  -- during: y.start < x.start ∧ x.stop < y.stop
  · obtain ⟨hss, hee⟩ := h
    have : ¬(x.start < y.start) := fun hc => TimePoint.lt_antisymm hc hss
    have : ¬(x.start = y.start) := fun heq => by rw [← heq] at hss; exact TimePoint.lt_irrefl _ hss
    -- y.stop > x.stop ≥ x.start, so y.stop > x.start
    have : ¬(y.stop < x.start) := fun hc =>
      TimePoint.lt_antisymm hc (TimePoint.lt_of_le_of_lt' x.valid hee)
    have : ¬(y.stop = x.start) := fun heq => by
      rw [heq] at hee  -- hee : x.stop < x.start
      exact TimePoint.lt_irrefl _ (TimePoint.lt_of_le_of_lt' x.valid hee)
    split <;> first | rfl | contradiction
  -- contains: x.start < y.start ∧ y.stop < x.stop
  · obtain ⟨hss, hee⟩ := h
    -- x.stop > y.stop ≥ y.start, so x.stop > y.start
    have : ¬(x.stop < y.start) := fun hc =>
      TimePoint.lt_antisymm hc (TimePoint.lt_of_le_of_lt' y.valid hee)
    have : ¬(x.stop = y.start) := fun heq => by
      rw [heq] at hee; exact TimePoint.lt_irrefl _ (TimePoint.lt_of_le_of_lt' y.valid hee)
    have : ¬(x.stop < y.stop) := fun hc => TimePoint.lt_antisymm hc hee
    have : ¬(x.stop = y.stop) := fun heq => by rw [heq] at hee; exact TimePoint.lt_irrefl _ hee
    split <;> first | rfl | contradiction
  -- finishes: x.stop = y.stop ∧ y.start < x.start
  · obtain ⟨heqe, hss⟩ := h
    have : ¬(x.start < y.start) := fun hc => TimePoint.lt_antisymm hc hss
    have : ¬(x.start = y.start) := fun heq => by rw [← heq] at hss; exact TimePoint.lt_irrefl _ hss
    -- y.stop = x.stop > x.start (since x proper: x.start < x.stop)
    -- So y.stop > x.start
    have : ¬(y.stop < x.start) := fun hc => by
      have := heqe ▸ hx  -- x.start < y.stop
      exact TimePoint.lt_antisymm hc this
    have : ¬(y.stop = x.start) := fun heq => by
      have := heqe ▸ hx  -- x.start < y.stop
      rw [heq] at this; exact TimePoint.lt_irrefl _ this
    have : ¬(x.stop < y.stop) := by rw [heqe]; exact fun hc => TimePoint.lt_irrefl _ hc
    split <;> first | rfl | contradiction
  -- finishedBy: x.stop = y.stop ∧ x.start < y.start
  · obtain ⟨heqe, hss⟩ := h
    have : ¬(x.stop < y.start) := fun hc => by
      rw [heqe] at hc; exact TimePoint.lt_antisymm hc hy
    have : ¬(x.stop = y.start) := fun heq => by
      rw [heqe] at heq; rw [← heq] at hy; exact TimePoint.lt_irrefl _ hy
    have : ¬(x.stop < y.stop) := by rw [heqe]; exact fun hc => TimePoint.lt_irrefl _ hc
    split <;> first | rfl | contradiction
  -- equals: x.start = y.start ∧ x.stop = y.stop
  · obtain ⟨heqs, heqe⟩ := h
    have : ¬(x.start < y.start) := by rw [heqs]; exact fun hc => TimePoint.lt_irrefl _ hc
    have : ¬(x.stop < y.stop) := by rw [heqe]; exact fun hc => TimePoint.lt_irrefl _ hc
    split <;> first | rfl | contradiction

end Spindle.Arith
