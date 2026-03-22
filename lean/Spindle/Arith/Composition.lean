/-
  Allen Interval Relation Composition Table  (Allen 1983)

  Defines `compose : AllenRelation → AllenRelation → List AllenRelation`.
  Proves soundness: if r1.holds x y and r2.holds y z then classify x z ∈ compose r1 r2.

  Table entries were cross-checked by exhaustive integer enumeration.

  Sorry count: 0.  All 169 cases (13×13) of compose_sound are fully proved.
-/
import Spindle.Arith.AllenRelation

namespace Spindle.Arith

open AllenRelation

/-! ## Composition Table -/

def AllenRelation.compose : AllenRelation → AllenRelation → List AllenRelation
  | .before,       .before       => [.before]
  | .before,       .meets        => [.before]
  | .before,       .overlaps     => [.before]
  | .before,       .starts       => [.before]
  | .before,       .during       => [.before, .meets, .overlaps, .starts, .during]
  | .before,       .finishes     => [.before, .meets, .overlaps, .starts, .during]
  | .before,       .equals       => [.before]
  | .before,       .finishedBy   => [.before]
  | .before,       .contains     => [.before]
  | .before,       .startedBy    => [.before]
  | .before,       .overlappedBy => [.before, .meets, .overlaps, .starts, .during]
  | .before,       .metBy        => [.before, .meets, .overlaps, .starts, .during]
  | .before,       .after        => [.before, .meets, .overlaps, .starts, .during,
                                     .equals, .finishes, .finishedBy, .contains,
                                     .startedBy, .overlappedBy, .metBy, .after]
  | .meets,        .before       => [.before]
  | .meets,        .meets        => [.before]
  | .meets,        .overlaps     => [.before]
  | .meets,        .starts       => [.meets]
  | .meets,        .during       => [.overlaps, .starts, .during]
  | .meets,        .finishes     => [.overlaps, .starts, .during]
  | .meets,        .equals       => [.meets]
  | .meets,        .finishedBy   => [.before]
  | .meets,        .contains     => [.before]
  | .meets,        .startedBy    => [.meets]
  | .meets,        .overlappedBy => [.overlaps, .starts, .during]
  | .meets,        .metBy        => [.equals, .finishedBy, .finishes]
  | .meets,        .after        => [.after, .contains, .metBy, .overlappedBy, .startedBy]
  | .overlaps,     .before       => [.before]
  | .overlaps,     .meets        => [.before]
  | .overlaps,     .overlaps     => [.before, .meets, .overlaps]
  | .overlaps,     .starts       => [.overlaps]
  | .overlaps,     .during       => [.overlaps, .starts, .during]
  | .overlaps,     .finishes     => [.overlaps, .starts, .during]
  | .overlaps,     .equals       => [.overlaps]
  | .overlaps,     .finishedBy   => [.before, .meets, .overlaps]
  | .overlaps,     .contains     => [.before, .meets, .overlaps, .finishedBy, .contains]
  | .overlaps,     .startedBy    => [.overlaps, .finishedBy, .contains]
  | .overlaps,     .overlappedBy => [.overlaps, .starts, .during, .equals, .finishes,
                                     .finishedBy, .contains, .startedBy, .overlappedBy]
  | .overlaps,     .metBy        => [.overlappedBy, .startedBy, .contains]
  | .overlaps,     .after        => [.after, .metBy, .overlappedBy, .startedBy, .contains]
  | .starts,       .before       => [.before]
  | .starts,       .meets        => [.before]
  | .starts,       .overlaps     => [.before, .meets, .overlaps]
  | .starts,       .starts       => [.starts]
  | .starts,       .during       => [.during]
  | .starts,       .finishes     => [.during]
  | .starts,       .equals       => [.starts]
  | .starts,       .finishedBy   => [.before, .meets, .overlaps]
  | .starts,       .contains     => [.before, .meets, .overlaps, .finishedBy, .contains]
  | .starts,       .startedBy    => [.starts, .equals, .startedBy]
  | .starts,       .overlappedBy => [.during, .finishes, .overlappedBy]
  | .starts,       .metBy        => [.metBy]
  | .starts,       .after        => [.after]
  | .during,       .before       => [.before]
  | .during,       .meets        => [.before]
  | .during,       .overlaps     => [.before, .meets, .overlaps, .starts, .during]
  | .during,       .starts       => [.during]
  | .during,       .during       => [.during]
  | .during,       .finishes     => [.during]
  | .during,       .equals       => [.during]
  | .during,       .finishedBy   => [.before, .meets, .overlaps, .starts, .during]
  | .during,       .contains     => [.before, .meets, .overlaps, .starts, .during,
                                     .equals, .finishes, .finishedBy, .contains,
                                     .startedBy, .overlappedBy, .metBy, .after]
  | .during,       .startedBy    => [.after, .metBy, .overlappedBy, .finishes, .during]
  | .during,       .overlappedBy => [.after, .metBy, .overlappedBy, .finishes, .during]
  | .during,       .metBy        => [.after]
  | .during,       .after        => [.after]
  | .finishes,     .before       => [.before]
  | .finishes,     .meets        => [.meets]
  | .finishes,     .overlaps     => [.overlaps, .starts, .during]
  | .finishes,     .starts       => [.during]
  | .finishes,     .during       => [.during]
  | .finishes,     .finishes     => [.finishes]
  | .finishes,     .equals       => [.finishes]
  | .finishes,     .finishedBy   => [.equals, .finishedBy, .finishes]
  | .finishes,     .contains     => [.after, .metBy, .overlappedBy, .startedBy, .contains]
  | .finishes,     .startedBy    => [.after, .metBy, .overlappedBy]
  | .finishes,     .overlappedBy => [.after, .metBy, .overlappedBy]
  | .finishes,     .metBy        => [.after]
  | .finishes,     .after        => [.after]
  | .equals,       .before       => [.before]
  | .equals,       .meets        => [.meets]
  | .equals,       .overlaps     => [.overlaps]
  | .equals,       .starts       => [.starts]
  | .equals,       .during       => [.during]
  | .equals,       .finishes     => [.finishes]
  | .equals,       .equals       => [.equals]
  | .equals,       .finishedBy   => [.finishedBy]
  | .equals,       .contains     => [.contains]
  | .equals,       .startedBy    => [.startedBy]
  | .equals,       .overlappedBy => [.overlappedBy]
  | .equals,       .metBy        => [.metBy]
  | .equals,       .after        => [.after]
  | .finishedBy,   .before       => [.before]
  | .finishedBy,   .meets        => [.meets]
  | .finishedBy,   .overlaps     => [.overlaps]
  | .finishedBy,   .starts       => [.overlaps]
  | .finishedBy,   .during       => [.overlaps, .starts, .during]
  | .finishedBy,   .finishes     => [.equals, .finishedBy, .finishes]
  | .finishedBy,   .equals       => [.finishedBy]
  | .finishedBy,   .finishedBy   => [.finishedBy]
  | .finishedBy,   .contains     => [.contains]
  | .finishedBy,   .startedBy    => [.contains]
  | .finishedBy,   .overlappedBy => [.overlappedBy, .startedBy, .contains]
  | .finishedBy,   .metBy        => [.overlappedBy, .startedBy, .contains]
  | .finishedBy,   .after        => [.after, .metBy, .overlappedBy, .startedBy, .contains]
  | .contains,     .before       => [.before, .meets, .overlaps, .finishedBy, .contains]
  | .contains,     .meets        => [.overlaps, .finishedBy, .contains]
  | .contains,     .overlaps     => [.overlaps, .finishedBy, .contains]
  | .contains,     .starts       => [.overlaps, .finishedBy, .contains]
  | .contains,     .during       => [.overlaps, .starts, .during, .equals, .finishes,
                                     .finishedBy, .contains, .startedBy, .overlappedBy]
  | .contains,     .finishes     => [.overlappedBy, .startedBy, .contains]
  | .contains,     .equals       => [.contains]
  | .contains,     .finishedBy   => [.contains]
  | .contains,     .contains     => [.contains]
  | .contains,     .startedBy    => [.contains]
  | .contains,     .overlappedBy => [.overlappedBy, .startedBy, .contains]
  | .contains,     .metBy        => [.overlappedBy, .startedBy, .contains]
  | .contains,     .after        => [.after, .metBy, .overlappedBy, .startedBy, .contains]
  | .startedBy,    .before       => [.before, .meets, .overlaps, .finishedBy, .contains]
  | .startedBy,    .meets        => [.overlaps, .finishedBy, .contains]
  | .startedBy,    .overlaps     => [.overlaps, .finishedBy, .contains]
  | .startedBy,    .starts       => [.starts, .equals, .startedBy]
  | .startedBy,    .during       => [.during, .finishes, .overlappedBy]
  | .startedBy,    .finishes     => [.overlappedBy]
  | .startedBy,    .equals       => [.startedBy]
  | .startedBy,    .finishedBy   => [.contains]
  | .startedBy,    .contains     => [.contains]
  | .startedBy,    .startedBy    => [.startedBy]
  | .startedBy,    .overlappedBy => [.overlappedBy]
  | .startedBy,    .metBy        => [.metBy]
  | .startedBy,    .after        => [.after]
  | .overlappedBy, .before       => [.before, .meets, .overlaps, .finishedBy, .contains]
  | .overlappedBy, .meets        => [.overlaps, .finishedBy, .contains]
  | .overlappedBy, .overlaps     => [.overlaps, .starts, .during, .equals, .finishes,
                                     .finishedBy, .contains, .startedBy, .overlappedBy]
  | .overlappedBy, .starts       => [.during, .finishes, .overlappedBy]
  | .overlappedBy, .during       => [.during, .finishes, .overlappedBy]
  | .overlappedBy, .finishes     => [.overlappedBy]
  | .overlappedBy, .equals       => [.overlappedBy]
  | .overlappedBy, .finishedBy   => [.overlappedBy, .startedBy, .contains]
  | .overlappedBy, .contains     => [.after, .metBy, .overlappedBy, .startedBy, .contains]
  | .overlappedBy, .startedBy    => [.after, .metBy, .overlappedBy]
  | .overlappedBy, .overlappedBy => [.after, .metBy, .overlappedBy]
  | .overlappedBy, .metBy        => [.after]
  | .overlappedBy, .after        => [.after]
  | .metBy,        .before       => [.before, .meets, .overlaps, .finishedBy, .contains]
  | .metBy,        .meets        => [.starts, .equals, .startedBy]
  | .metBy,        .overlaps     => [.during, .finishes, .overlappedBy]
  | .metBy,        .starts       => [.during, .finishes, .overlappedBy]
  | .metBy,        .during       => [.during, .finishes, .overlappedBy]
  | .metBy,        .finishes     => [.metBy]
  | .metBy,        .equals       => [.metBy]
  | .metBy,        .finishedBy   => [.metBy]
  | .metBy,        .contains     => [.after]
  | .metBy,        .startedBy    => [.after]
  | .metBy,        .overlappedBy => [.after]
  | .metBy,        .metBy        => [.after]
  | .metBy,        .after        => [.after]
  | .after,        .before       => [.before, .meets, .overlaps, .starts, .during,
                                     .equals, .finishes, .finishedBy, .contains,
                                     .startedBy, .overlappedBy, .metBy, .after]
  | .after,        .meets        => [.after, .metBy, .overlappedBy, .finishes, .during]
  | .after,        .overlaps     => [.after, .metBy, .overlappedBy, .finishes, .during]
  | .after,        .starts       => [.after, .metBy, .overlappedBy, .finishes, .during]
  | .after,        .during       => [.after, .metBy, .overlappedBy, .finishes, .during]
  | .after,        .finishes     => [.after]
  | .after,        .equals       => [.after]
  | .after,        .finishedBy   => [.after]
  | .after,        .contains     => [.after]
  | .after,        .startedBy    => [.after]
  | .after,        .overlappedBy => [.after]
  | .after,        .metBy        => [.after]
  | .after,        .after        => [.after]

/-! ## Classify-result lemmas -/

private theorem cl_before {x z : Interval} (h1 : x.start < z.start) (h2 : x.stop < z.start) :
    classify x z = .before := by unfold classify; simp only [if_pos h1, if_pos h2]

private theorem cl_meets {x z : Interval} (h1 : x.start < z.start) (h2 : x.stop = z.start) :
    classify x z = .meets := by
  unfold classify
  have : ¬(x.stop < z.start) := by rw [h2]; exact TimePoint.lt_irrefl _
  simp only [if_pos h1, if_neg this, if_pos h2]

private theorem cl_overlaps {x z : Interval} (h1 : x.start < z.start)
    (h2 : z.start < x.stop) (h3 : x.stop < z.stop) : classify x z = .overlaps := by
  unfold classify
  have neg2a : ¬(x.stop < z.start) := fun h => TimePoint.lt_antisymm h2 h
  have neg2b : x.stop ≠ z.start := fun e => TimePoint.lt_irrefl _ (e ▸ h2)
  simp only [if_pos h1, if_neg neg2a, if_neg neg2b, if_pos h3]

private theorem cl_finishedBy {x z : Interval} (h1 : x.start < z.start)
    (h2 : z.start < x.stop) (h3 : x.stop = z.stop) : classify x z = .finishedBy := by
  unfold classify
  have neg2a : ¬(x.stop < z.start) := fun h => TimePoint.lt_antisymm h2 h
  have neg2b : x.stop ≠ z.start := fun e => TimePoint.lt_irrefl _ (e ▸ h2)
  have neg3 : ¬(x.stop < z.stop) := by rw [h3]; exact TimePoint.lt_irrefl _
  simp only [if_pos h1, if_neg neg2a, if_neg neg2b, if_neg neg3, if_pos h3]

private theorem cl_contains {x z : Interval} (h1 : x.start < z.start)
    (h2 : z.start < x.stop) (h3 : z.stop < x.stop) : classify x z = .contains := by
  unfold classify
  have neg2a : ¬(x.stop < z.start) := fun h => TimePoint.lt_antisymm h2 h
  have neg2b : x.stop ≠ z.start := fun e => TimePoint.lt_irrefl _ (e ▸ h2)
  have neg3a : ¬(x.stop < z.stop) := fun h => TimePoint.lt_antisymm h3 h
  have neg3b : x.stop ≠ z.stop := fun e => TimePoint.lt_irrefl _ (e ▸ h3)
  simp only [if_pos h1, if_neg neg2a, if_neg neg2b, if_neg neg3a, if_neg neg3b]

private theorem cl_starts {x z : Interval} (h1 : x.start = z.start) (h2 : x.stop < z.stop) :
    classify x z = .starts := by
  unfold classify
  have : ¬(x.start < z.start) := by rw [h1]; exact TimePoint.lt_irrefl _
  simp only [if_neg this, if_pos h1, if_pos h2]

private theorem cl_equals {x z : Interval} (h1 : x.start = z.start) (h2 : x.stop = z.stop) :
    classify x z = .equals := by
  unfold classify
  have neg1 : ¬(x.start < z.start) := by rw [h1]; exact TimePoint.lt_irrefl _
  have neg2 : ¬(x.stop < z.stop) := by rw [h2]; exact TimePoint.lt_irrefl _
  simp only [if_neg neg1, if_pos h1, if_neg neg2, if_pos h2]

private theorem cl_startedBy {x z : Interval} (h1 : x.start = z.start) (h2 : z.stop < x.stop) :
    classify x z = .startedBy := by
  unfold classify
  have neg1 : ¬(x.start < z.start) := by rw [h1]; exact TimePoint.lt_irrefl _
  have neg2a : ¬(x.stop < z.stop) := fun h => TimePoint.lt_antisymm h2 h
  have neg2b : x.stop ≠ z.stop := fun e => TimePoint.lt_irrefl _ (e ▸ h2)
  simp only [if_neg neg1, if_pos h1, if_neg neg2a, if_neg neg2b]

private theorem cl_after {x z : Interval} (h1 : z.start < x.start) (h2 : z.stop < x.start) :
    classify x z = .after := by
  unfold classify
  have neg1a : ¬(x.start < z.start) := fun h => TimePoint.lt_antisymm h1 h
  have neg1b : x.start ≠ z.start := fun e => TimePoint.lt_irrefl _ (e ▸ h1)
  simp only [if_neg neg1a, if_neg neg1b, if_pos h2]

private theorem cl_metBy {x z : Interval} (h1 : z.start < x.start) (h2 : z.stop = x.start) :
    classify x z = .metBy := by
  unfold classify
  have neg1a : ¬(x.start < z.start) := fun h => TimePoint.lt_antisymm h1 h
  have neg1b : x.start ≠ z.start := fun e => TimePoint.lt_irrefl _ (e ▸ h1)
  have neg2 : ¬(z.stop < x.start) := by rw [h2]; exact TimePoint.lt_irrefl _
  simp only [if_neg neg1a, if_neg neg1b, if_neg neg2, if_pos h2]

private theorem cl_during {x z : Interval} (h1 : z.start < x.start)
    (h2 : x.start < z.stop) (h3 : x.stop < z.stop) : classify x z = .during := by
  unfold classify
  have neg1a : ¬(x.start < z.start) := fun h => TimePoint.lt_antisymm h1 h
  have neg1b : x.start ≠ z.start := fun e => TimePoint.lt_irrefl _ (e ▸ h1)
  have neg2a : ¬(z.stop < x.start) := fun h => TimePoint.lt_antisymm h2 h
  have neg2b : z.stop ≠ x.start := fun e => TimePoint.lt_irrefl _ (e ▸ h2)
  simp only [if_neg neg1a, if_neg neg1b, if_neg neg2a, if_neg neg2b, if_pos h3]

private theorem cl_finishes {x z : Interval} (h1 : z.start < x.start)
    (h2 : x.start < z.stop) (h3 : x.stop = z.stop) : classify x z = .finishes := by
  unfold classify
  have neg1a : ¬(x.start < z.start) := fun h => TimePoint.lt_antisymm h1 h
  have neg1b : x.start ≠ z.start := fun e => TimePoint.lt_irrefl _ (e ▸ h1)
  have neg2a : ¬(z.stop < x.start) := fun h => TimePoint.lt_antisymm h2 h
  have neg2b : z.stop ≠ x.start := fun e => TimePoint.lt_irrefl _ (e ▸ h2)
  have neg3 : ¬(x.stop < z.stop) := by rw [h3]; exact TimePoint.lt_irrefl _
  simp only [if_neg neg1a, if_neg neg1b, if_neg neg2a, if_neg neg2b, if_neg neg3, if_pos h3]

private theorem cl_overlappedBy {x z : Interval} (h1 : z.start < x.start)
    (h2 : x.start < z.stop) (h3 : z.stop < x.stop) : classify x z = .overlappedBy := by
  unfold classify
  have neg1a : ¬(x.start < z.start) := fun h => TimePoint.lt_antisymm h1 h
  have neg1b : x.start ≠ z.start := fun e => TimePoint.lt_irrefl _ (e ▸ h1)
  have neg2a : ¬(z.stop < x.start) := fun h => TimePoint.lt_antisymm h2 h
  have neg2b : z.stop ≠ x.start := fun e => TimePoint.lt_irrefl _ (e ▸ h2)
  have neg3a : ¬(x.stop < z.stop) := fun h => TimePoint.lt_antisymm h3 h
  have neg3b : x.stop ≠ z.stop := fun e => TimePoint.lt_irrefl _ (e ▸ h3)
  simp only [if_neg neg1a, if_neg neg1b, if_neg neg2a, if_neg neg2b, if_neg neg3a, if_neg neg3b]

/-! ## Soundness proof -/

-- Shorthand for lt_trans chaining
private abbrev ltr {a b c : TimePoint} (h1 : a < b) (h2 : b < c) : a < c :=
  TimePoint.lt_trans h1 h2

-- Helper: rewrite classify to a known value, then prove membership
private theorem mem_cl {x z : Interval} {l : List AllenRelation}
    {r : AllenRelation} (hcl : classify x z = r) (hmem : r ∈ l) : classify x z ∈ l :=
  hcl ▸ hmem

-- Shorthand to prove classify x z ∈ compose r1 r2 in a single cell
-- Usage: by_cl (cl_before hss hse) (List.mem_cons_self _ _)
--   i.e. by_cl <classify-proof> <membership-proof>
private theorem by_cl_thm {x z : Interval} {r1 r2 : AllenRelation} {r : AllenRelation}
    (hcl : classify x z = r) (hmem : r ∈ compose r1 r2) : classify x z ∈ compose r1 r2 :=
  hcl ▸ hmem

set_option maxHeartbeats 800000 in
theorem AllenRelation.compose_sound
    (x y z : Interval) (hx : x.proper) (hy : y.proper) (hz : z.proper)
    (r1 r2 : AllenRelation) (h1 : r1.holds x y) (h2 : r2.holds y z) :
    (classify x z) ∈ compose r1 r2 := by
  unfold Interval.proper at hx hy hz
  simp only [holds] at h1 h2
  match r1, r2 with

  -- ════════ r1 = before ════════
  | .before, .before =>
    have hse : x.stop < z.start := ltr (ltr h1 hy) h2
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .before, .meets =>
    have hse : x.stop < z.start := h2 ▸ ltr h1 hy
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .before, .overlaps =>
    have hse : x.stop < z.start := ltr h1 h2.1
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .before, .starts =>
    have hse : x.stop < z.start := h2.1 ▸ h1
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .before, .finishedBy =>
    have hse : x.stop < z.start := ltr h1 h2.2
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .before, .contains =>
    have hse : x.stop < z.start := ltr h1 h2.1
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .before, .startedBy =>
    have hse : x.stop < z.start := h2.1 ▸ h1
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .before, .equals =>
    have hse : x.stop < z.start := h2.1 ▸ h1
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .before, .during =>
    -- h2 : z.start < y.start ∧ y.stop < z.stop
    have hxs : x.stop < z.stop := ltr (ltr h1 hy) h2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
      · rw [cl_before hss hse]; simp [List.mem_cons]
      · rw [cl_meets hss hse]; simp [List.mem_cons]
      · rw [cl_overlaps hss hse hxs]; simp [List.mem_cons]
    · rw [cl_starts hss hxs]; simp [List.mem_cons]
    · rw [cl_during hss (ltr hx hxs) hxs]; simp [List.mem_cons]
  | .before, .finishes =>
    -- h2 : y.stop = z.stop ∧ z.start < y.start
    have hxs : x.stop < z.stop := h2.1 ▸ ltr h1 hy
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
      · rw [cl_before hss hse]; simp [List.mem_cons]
      · rw [cl_meets hss hse]; simp [List.mem_cons]
      · rw [cl_overlaps hss hse hxs]; simp [List.mem_cons]
    · rw [cl_starts hss hxs]; simp [List.mem_cons]
    · rw [cl_during hss (ltr hx hxs) hxs]; simp [List.mem_cons]
  | .before, .overlappedBy =>
    -- h2 : z.start < y.start ∧ y.start < z.stop ∧ z.stop < y.stop
    have hxs : x.stop < z.stop := ltr h1 h2.2.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
      · rw [cl_before hss hse]; simp [List.mem_cons]
      · rw [cl_meets hss hse]; simp [List.mem_cons]
      · rw [cl_overlaps hss hse hxs]; simp [List.mem_cons]
    · rw [cl_starts hss hxs]; simp [List.mem_cons]
    · rw [cl_during hss (ltr hx hxs) hxs]; simp [List.mem_cons]
  | .before, .metBy =>
    -- h2 : z.stop = y.start => x.stop < z.stop
    have hxs : x.stop < z.stop := h2 ▸ h1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
      · rw [cl_before hss hse]; simp [List.mem_cons]
      · rw [cl_meets hss hse]; simp [List.mem_cons]
      · rw [cl_overlaps hss hse hxs]; simp [List.mem_cons]
    · rw [cl_starts hss hxs]; simp [List.mem_cons]
    · rw [cl_during hss (ltr hx hxs) hxs]; simp [List.mem_cons]
  | .before, .after =>
    -- all 13 possible
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
      · rw [cl_before hss hse]; simp [List.mem_cons]
      · rw [cl_meets hss hse]; simp [List.mem_cons]
      · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
        · rw [cl_overlaps hss hse hee]; simp [List.mem_cons]
        · rw [cl_finishedBy hss hse hee]; simp [List.mem_cons]
        · rw [cl_contains hss hse hee]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_starts hss hee]; simp [List.mem_cons]
      · rw [cl_equals hss hee]; simp [List.mem_cons]
      · rw [cl_startedBy hss hee]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
      · rw [cl_after hss hse]; simp [List.mem_cons]
      · rw [cl_metBy hss hse]; simp [List.mem_cons]
      · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
        · rw [cl_during hss hse hee]; simp [List.mem_cons]
        · rw [cl_finishes hss hse hee]; simp [List.mem_cons]
        · rw [cl_overlappedBy hss hse hee]; simp [List.mem_cons]

  -- ════════ r1 = meets: h1 : x.stop = y.start ════════
  | .meets, .before =>
    have hse : x.stop < z.start := h1 ▸ ltr hy h2
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .meets, .meets =>
    have hse : x.stop < z.start := h2 ▸ (h1 ▸ hy)
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .meets, .overlaps =>
    have hse : x.stop < z.start := h1 ▸ h2.1
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .meets, .finishedBy =>
    have hse : x.stop < z.start := h1 ▸ h2.2
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .meets, .contains =>
    have hse : x.stop < z.start := h1 ▸ h2.1
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .meets, .starts =>
    have heq : x.stop = z.start := h1.trans h2.1
    simp only [compose]; exact mem_cl (cl_meets (heq ▸ hx) heq) (by simp [List.mem_cons])
  | .meets, .startedBy =>
    have heq : x.stop = z.start := h1.trans h2.1
    simp only [compose]; exact mem_cl (cl_meets (heq ▸ hx) heq) (by simp [List.mem_cons])
  | .meets, .equals =>
    have heq : x.stop = z.start := h1.trans h2.1
    simp only [compose]; exact mem_cl (cl_meets (heq ▸ hx) heq) (by simp [List.mem_cons])
  | .meets, .during =>
    -- z.start < y.start = x.stop; x.stop < y.stop < z.stop
    have hzs_xe : z.start < x.stop := h1 ▸ h2.1
    have hxs : x.stop < z.stop := h1 ▸ ltr hy h2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_overlaps hss hzs_xe hxs]; simp [List.mem_cons]
    · rw [cl_starts hss hxs]; simp [List.mem_cons]
    · rw [cl_during hss (ltr hx hxs) hxs]; simp [List.mem_cons]
  | .meets, .finishes =>
    have hzs_xe : z.start < x.stop := h1 ▸ h2.2
    have hxs : x.stop < z.stop := h2.1 ▸ (h1 ▸ hy)
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_overlaps hss hzs_xe hxs]; simp [List.mem_cons]
    · rw [cl_starts hss hxs]; simp [List.mem_cons]
    · rw [cl_during hss (ltr hx hxs) hxs]; simp [List.mem_cons]
  | .meets, .overlappedBy =>
    have hzs_xe : z.start < x.stop := h1 ▸ h2.1
    have hxs : x.stop < z.stop := h1 ▸ h2.2.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_overlaps hss hzs_xe hxs]; simp [List.mem_cons]
    · rw [cl_starts hss hxs]; simp [List.mem_cons]
    · rw [cl_during hss (ltr hx hxs) hxs]; simp [List.mem_cons]
  | .meets, .metBy =>
    -- z.stop = y.start = x.stop => x.stop = z.stop
    have heq : x.stop = z.stop := h1.trans h2.symm
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_finishedBy hss (heq ▸ hz) heq]; simp [List.mem_cons]
    · rw [cl_equals hss heq]; simp [List.mem_cons]
    · rw [cl_finishes hss (heq ▸ hx) heq]; simp [List.mem_cons]
  | .meets, .after =>
    -- z.stop < y.start = x.stop
    have hze_xs : z.stop < x.stop := h1 ▸ h2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_contains hss (ltr hz hze_xs) hze_xs]; simp [List.mem_cons]
    · rw [cl_startedBy hss hze_xs]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
      · rw [cl_after hss hse]; simp [List.mem_cons]
      · rw [cl_metBy hss hse]; simp [List.mem_cons]
      · rw [cl_overlappedBy hss hse hze_xs]; simp [List.mem_cons]

  -- ════════ r1 = overlaps: h1 : x.start < y.start ∧ y.start < x.stop ∧ x.stop < y.stop ════════
  | .overlaps, .before =>
    have hse : x.stop < z.start := ltr h1.2.2 h2
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .overlaps, .meets =>
    have hse : x.stop < z.start := h2 ▸ h1.2.2
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .overlaps, .overlaps =>
    -- x.start < y.start < z.start; x.stop < z.stop
    have hss : x.start < z.start := ltr h1.1 h2.1
    have hxs : x.stop < z.stop := ltr h1.2.2 h2.2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
    · rw [cl_before hss hse]; simp [List.mem_cons]
    · rw [cl_meets hss hse]; simp [List.mem_cons]
    · rw [cl_overlaps hss hse hxs]; simp [List.mem_cons]
  | .overlaps, .starts =>
    -- x.start < y.start = z.start; y.start < x.stop; x.stop < y.stop < z.stop
    have hss : x.start < z.start := h2.1 ▸ h1.1
    have hzs_xe : z.start < x.stop := h2.1 ▸ h1.2.1
    have hxs : x.stop < z.stop := ltr h1.2.2 h2.2
    simp only [compose]; exact mem_cl (cl_overlaps hss hzs_xe hxs) (by simp [List.mem_cons])
  | .overlaps, .during =>
    -- h2: z.start < y.start; x.start < y.start; x.stop < z.stop
    have hxs : x.stop < z.stop := ltr h1.2.2 h2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_overlaps hss (ltr h2.1 h1.2.1) hxs]; simp [List.mem_cons]
    · rw [cl_starts hss hxs]; simp [List.mem_cons]
    · rw [cl_during hss (ltr hx hxs) hxs]; simp [List.mem_cons]
  | .overlaps, .finishes =>
    -- h2: y.stop = z.stop ∧ z.start < y.start
    have hxs : x.stop < z.stop := h2.1 ▸ h1.2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_overlaps hss (ltr h2.2 h1.2.1) hxs]; simp [List.mem_cons]
    · rw [cl_starts hss hxs]; simp [List.mem_cons]
    · rw [cl_during hss (ltr hx hxs) hxs]; simp [List.mem_cons]
  | .overlaps, .equals =>
    -- h2: y.start = z.start ∧ y.stop = z.stop
    have hss : x.start < z.start := h2.1 ▸ h1.1
    have hzs_xe : z.start < x.stop := h2.1 ▸ h1.2.1
    have hxs : x.stop < z.stop := h2.2 ▸ h1.2.2
    simp only [compose]; exact mem_cl (cl_overlaps hss hzs_xe hxs) (by simp [List.mem_cons])
  | .overlaps, .finishedBy =>
    -- h2: y.stop = z.stop ∧ y.start < z.start; x.start < y.start < z.start; x.stop < y.pop = z.stop
    have hss : x.start < z.start := ltr h1.1 h2.2
    have hxs : x.stop < z.stop := h2.1 ▸ h1.2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
    · rw [cl_before hss hse]; simp [List.mem_cons]
    · rw [cl_meets hss hse]; simp [List.mem_cons]
    · rw [cl_overlaps hss hse hxs]; simp [List.mem_cons]
  | .overlaps, .contains =>
    -- h2: y.start < z.start ∧ z.stop < y.pop; x.start < y.start < z.start
    have hss : x.start < z.start := ltr h1.1 h2.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
    · rw [cl_before hss hse]; simp [List.mem_cons]
    · rw [cl_meets hss hse]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_overlaps hss hse hee]; simp [List.mem_cons]
      · rw [cl_finishedBy hss hse hee]; simp [List.mem_cons]
      · rw [cl_contains hss hse hee]; simp [List.mem_cons]
  | .overlaps, .startedBy =>
    -- h2: y.start = z.start ∧ z.stop < y.pop; x.start < z.start; z.start < x.stop
    have hss : x.start < z.start := h2.1 ▸ h1.1
    have hzs_xe : z.start < x.stop := h2.1 ▸ h1.2.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_overlaps hss hzs_xe hee]; simp [List.mem_cons]
    · rw [cl_finishedBy hss hzs_xe hee]; simp [List.mem_cons]
    · rw [cl_contains hss hzs_xe hee]; simp [List.mem_cons]
  | .overlaps, .overlappedBy =>
    -- h2: z.start < y.start ∧ y.start < z.stop ∧ z.stop < y.pop
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_overlaps hss (ltr h2.1 h1.2.1) hee]; simp [List.mem_cons]
      · rw [cl_finishedBy hss (ltr h2.1 h1.2.1) hee]; simp [List.mem_cons]
      · rw [cl_contains hss (ltr h2.1 h1.2.1) hee]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_starts hss hee]; simp [List.mem_cons]
      · rw [cl_equals hss hee]; simp [List.mem_cons]
      · rw [cl_startedBy hss hee]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_during hss (ltr h1.1 h2.2.1) hee]; simp [List.mem_cons]
      · rw [cl_finishes hss (ltr h1.1 h2.2.1) hee]; simp [List.mem_cons]
      · rw [cl_overlappedBy hss (ltr h1.1 h2.2.1) hee]; simp [List.mem_cons]
  | .overlaps, .metBy =>
    -- h2: z.stop = y.start; z.stop < x.stop; x.start < z.stop
    have hze_xs : z.stop < x.stop := h2 ▸ h1.2.1
    have hxst_ze : x.start < z.stop := h2 ▸ h1.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_contains hss (ltr hz hze_xs) hze_xs]; simp [List.mem_cons]
    · rw [cl_startedBy hss hze_xs]; simp [List.mem_cons]
    · rw [cl_overlappedBy hss hxst_ze hze_xs]; simp [List.mem_cons]
  | .overlaps, .after =>
    -- h2: z.stop < y.start; z.stop < x.stop
    have hze_xs : z.stop < x.stop := ltr h2 h1.2.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_contains hss (ltr hz hze_xs) hze_xs]; simp [List.mem_cons]
    · rw [cl_startedBy hss hze_xs]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
      · rw [cl_after hss hse]; simp [List.mem_cons]
      · rw [cl_metBy hss hse]; simp [List.mem_cons]
      · rw [cl_overlappedBy hss hse hze_xs]; simp [List.mem_cons]

  -- ════════ r1 = starts: h1 : x.start = y.start ∧ x.stop < y.stop ════════
  | .starts, .before =>
    have hse : x.stop < z.start := ltr h1.2 h2
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .starts, .meets =>
    have hse : x.stop < z.start := h2 ▸ h1.2
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .starts, .overlaps =>
    have hss : x.start < z.start := h1.1 ▸ h2.1
    have hxs : x.stop < z.stop := ltr h1.2 h2.2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
    · rw [cl_before hss hse]; simp [List.mem_cons]
    · rw [cl_meets hss hse]; simp [List.mem_cons]
    · rw [cl_overlaps hss hse hxs]; simp [List.mem_cons]
  | .starts, .starts =>
    have hss : x.start = z.start := h1.1.trans h2.1
    simp only [compose]; exact mem_cl (cl_starts hss (ltr h1.2 h2.2)) (by simp [List.mem_cons])
  | .starts, .during =>
    -- h2: z.start < y.start = x.start; x.stop < z.stop
    have hzs_xa : z.start < x.start := h1.1 ▸ h2.1
    have hxs : x.stop < z.stop := ltr h1.2 h2.2
    simp only [compose]; exact mem_cl (cl_during hzs_xa (ltr hx hxs) hxs) (by simp [List.mem_cons])
  | .starts, .finishes =>
    -- h2: y.pop = z.stop ∧ z.start < y.start = x.start
    have hzs_xa : z.start < x.start := h1.1 ▸ h2.2
    have hxs : x.stop < z.stop := h2.1 ▸ h1.2
    simp only [compose]; exact mem_cl (cl_during hzs_xa (ltr hx hxs) hxs) (by simp [List.mem_cons])
  | .starts, .equals =>
    have hss : x.start = z.start := h1.1.trans h2.1
    simp only [compose]; exact mem_cl (cl_starts hss (h2.2 ▸ h1.2)) (by simp [List.mem_cons])
  | .starts, .finishedBy =>
    -- h2: y.pop = z.stop ∧ y.start < z.start; x.start = y.start < z.start; x.stop < z.stop
    have hss : x.start < z.start := h1.1 ▸ h2.2
    have hxs : x.stop < z.stop := h2.1 ▸ h1.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
    · rw [cl_before hss hse]; simp [List.mem_cons]
    · rw [cl_meets hss hse]; simp [List.mem_cons]
    · rw [cl_overlaps hss hse hxs]; simp [List.mem_cons]
  | .starts, .contains =>
    -- h2: y.start < z.start ∧ z.stop < y.pop; x.start = y.start < z.start
    have hss : x.start < z.start := h1.1 ▸ h2.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
    · rw [cl_before hss hse]; simp [List.mem_cons]
    · rw [cl_meets hss hse]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_overlaps hss hse hee]; simp [List.mem_cons]
      · rw [cl_finishedBy hss hse hee]; simp [List.mem_cons]
      · rw [cl_contains hss hse hee]; simp [List.mem_cons]
  | .starts, .startedBy =>
    -- h2: y.start = z.start ∧ z.stop < y.pop; x.start = y.start = z.start
    have hss : x.start = z.start := h1.1.trans h2.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_starts hss hee]; simp [List.mem_cons]
    · rw [cl_equals hss hee]; simp [List.mem_cons]
    · rw [cl_startedBy hss hee]; simp [List.mem_cons]
  | .starts, .overlappedBy =>
    -- h2: z.start < y.start = x.start; x.start < z.stop; x.stop vs z.stop
    have hzs_xa : z.start < x.start := h1.1 ▸ h2.1
    have hxa_ze : x.start < z.stop := h1.1 ▸ h2.2.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_during hzs_xa hxa_ze hee]; simp [List.mem_cons]
    · rw [cl_finishes hzs_xa hxa_ze hee]; simp [List.mem_cons]
    · rw [cl_overlappedBy hzs_xa hxa_ze hee]; simp [List.mem_cons]
  | .starts, .metBy =>
    -- h2: z.stop = y.start = x.start
    have heq : z.stop = x.start := h2.trans h1.1.symm
    simp only [compose]; exact mem_cl (cl_metBy (heq ▸ hz) heq) (by simp [List.mem_cons])
  | .starts, .after =>
    -- h2: z.stop < y.start = x.start
    have hze_xa : z.stop < x.start := h1.1 ▸ h2
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])

  -- ════════ r1 = during: h1 : y.start < x.start ∧ x.stop < y.pop ════════
  | .during, .before =>
    have hse : x.stop < z.start := ltr h1.2 h2
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .during, .meets =>
    have hse : x.stop < z.start := h2 ▸ h1.2
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .during, .overlaps =>
    -- h2: y.start < z.start ∧ z.start < y.pop ∧ y.pop < z.stop
    have hxs : x.stop < z.stop := ltr h1.2 h2.2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
      · rw [cl_before hss hse]; simp [List.mem_cons]
      · rw [cl_meets hss hse]; simp [List.mem_cons]
      · rw [cl_overlaps hss hse hxs]; simp [List.mem_cons]
    · rw [cl_starts hss hxs]; simp [List.mem_cons]
    · rw [cl_during hss (ltr hx hxs) hxs]; simp [List.mem_cons]
  | .during, .starts =>
    have hzs_xa : z.start < x.start := h2.1 ▸ h1.1
    have hxs : x.stop < z.stop := ltr h1.2 h2.2
    simp only [compose]; exact mem_cl (cl_during hzs_xa (ltr hx hxs) hxs) (by simp [List.mem_cons])
  | .during, .during =>
    have hzs_xa : z.start < x.start := ltr h2.1 h1.1
    have hxs : x.stop < z.stop := ltr h1.2 h2.2
    simp only [compose]; exact mem_cl (cl_during hzs_xa (ltr hx hxs) hxs) (by simp [List.mem_cons])
  | .during, .finishes =>
    have hzs_xa : z.start < x.start := ltr h2.2 h1.1
    have hxs : x.stop < z.stop := h2.1 ▸ h1.2
    simp only [compose]; exact mem_cl (cl_during hzs_xa (ltr hx hxs) hxs) (by simp [List.mem_cons])
  | .during, .equals =>
    have hzs_xa : z.start < x.start := h2.1 ▸ h1.1
    have hxs : x.stop < z.stop := h2.2 ▸ h1.2
    simp only [compose]; exact mem_cl (cl_during hzs_xa (ltr hx hxs) hxs) (by simp [List.mem_cons])
  | .during, .finishedBy =>
    -- h2: y.pop = z.stop ∧ y.start < z.start; x.start vs z.start
    have hxs : x.stop < z.stop := h2.1 ▸ h1.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
      · rw [cl_before hss hse]; simp [List.mem_cons]
      · rw [cl_meets hss hse]; simp [List.mem_cons]
      · rw [cl_overlaps hss hse hxs]; simp [List.mem_cons]
    · rw [cl_starts hss hxs]; simp [List.mem_cons]
    · rw [cl_during hss (ltr hx hxs) hxs]; simp [List.mem_cons]
  | .during, .contains =>
    -- all 13
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
      · rw [cl_before hss hse]; simp [List.mem_cons]
      · rw [cl_meets hss hse]; simp [List.mem_cons]
      · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
        · rw [cl_overlaps hss hse hee]; simp [List.mem_cons]
        · rw [cl_finishedBy hss hse hee]; simp [List.mem_cons]
        · rw [cl_contains hss hse hee]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_starts hss hee]; simp [List.mem_cons]
      · rw [cl_equals hss hee]; simp [List.mem_cons]
      · rw [cl_startedBy hss hee]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
      · rw [cl_after hss hse]; simp [List.mem_cons]
      · rw [cl_metBy hss hse]; simp [List.mem_cons]
      · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
        · rw [cl_during hss hse hee]; simp [List.mem_cons]
        · rw [cl_finishes hss hse hee]; simp [List.mem_cons]
        · rw [cl_overlappedBy hss hse hee]; simp [List.mem_cons]
  | .during, .startedBy =>
    -- h2: y.start = z.start ∧ z.stop < y.pop; y.start < x.start => z.start < x.start
    have hzs_xa : z.start < x.start := h2.1 ▸ h1.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
    · rw [cl_after hzs_xa hse]; simp [List.mem_cons]
    · rw [cl_metBy hzs_xa hse]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_during hzs_xa hse hee]; simp [List.mem_cons]
      · rw [cl_finishes hzs_xa hse hee]; simp [List.mem_cons]
      · rw [cl_overlappedBy hzs_xa hse hee]; simp [List.mem_cons]
  | .during, .overlappedBy =>
    -- h2: z.start < y.start ∧ y.start < z.stop ∧ z.stop < y.stop; y.start < x.start
    have hzs_xa : z.start < x.start := ltr h2.1 h1.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
    · rw [cl_after hzs_xa hse]; simp [List.mem_cons]
    · rw [cl_metBy hzs_xa hse]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_during hzs_xa hse hee]; simp [List.mem_cons]
      · rw [cl_finishes hzs_xa hse hee]; simp [List.mem_cons]
      · rw [cl_overlappedBy hzs_xa hse hee]; simp [List.mem_cons]
  | .during, .metBy =>
    have hze_xa : z.stop < x.start := h2 ▸ h1.1
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])
  | .during, .after =>
    have hze_xa : z.stop < x.start := ltr h2 h1.1
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])

  -- ════════ r1 = finishes: h1 : x.stop = y.pop ∧ y.start < x.start ════════
  | .finishes, .before =>
    have hse : x.stop < z.start := h1.1 ▸ h2
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .finishes, .meets =>
    -- h2: y.pop = z.start; x.stop = y.pop = z.start
    have heq : x.stop = z.start := h1.1.trans h2
    simp only [compose]; exact mem_cl (cl_meets (heq ▸ hx) heq) (by simp [List.mem_cons])
  | .finishes, .overlaps =>
    -- h2: y.start < z.start ∧ z.start < y.pop ∧ y.pop < z.stop
    -- x.stop = y.pop; z.start < x.stop; x.stop < z.stop
    have hzs_xe : z.start < x.stop := h1.1 ▸ h2.2.1
    have hxs : x.stop < z.stop := h1.1 ▸ h2.2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_overlaps hss hzs_xe hxs]; simp [List.mem_cons]
    · rw [cl_starts hss hxs]; simp [List.mem_cons]
    · rw [cl_during hss (ltr hx hxs) hxs]; simp [List.mem_cons]
  | .finishes, .starts =>
    -- h2: y.start = z.start ∧ y.pop < z.stop; y.start < x.start => z.start < x.start
    have hzs_xa : z.start < x.start := h2.1 ▸ h1.2
    have hxs : x.stop < z.stop := h1.1 ▸ h2.2
    simp only [compose]; exact mem_cl (cl_during hzs_xa (ltr hx hxs) hxs) (by simp [List.mem_cons])
  | .finishes, .during =>
    -- h2: z.start < y.start ∧ y.pop < z.stop; y.start < x.start
    have hzs_xa : z.start < x.start := ltr h2.1 h1.2
    have hxs : x.stop < z.stop := h1.1 ▸ h2.2
    simp only [compose]; exact mem_cl (cl_during hzs_xa (ltr hx hxs) hxs) (by simp [List.mem_cons])
  | .finishes, .finishes =>
    -- h2: y.pop = z.stop ∧ z.start < y.start; x.stop = z.stop; z.start < x.start
    have hzs_xa : z.start < x.start := ltr h2.2 h1.2
    have heq : x.stop = z.stop := h1.1.trans h2.1
    simp only [compose]; exact mem_cl (cl_finishes hzs_xa (heq ▸ hx) heq) (by simp [List.mem_cons])
  | .finishes, .equals =>
    have hzs_xa : z.start < x.start := h2.1 ▸ h1.2
    have heq : x.stop = z.stop := h1.1.trans h2.2
    simp only [compose]; exact mem_cl (cl_finishes hzs_xa (heq ▸ hx) heq) (by simp [List.mem_cons])
  | .finishes, .finishedBy =>
    -- h2: y.pop = z.stop ∧ y.start < z.start; x.stop = z.stop; x.start vs z.start
    have heq : x.stop = z.stop := h1.1.trans h2.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_finishedBy hss (heq ▸ hz) heq]; simp [List.mem_cons]
    · rw [cl_equals hss heq]; simp [List.mem_cons]
    · rw [cl_finishes hss (heq ▸ hx) heq]; simp [List.mem_cons]
  | .finishes, .contains =>
    -- h2: y.start < z.start ∧ z.stop < y.pop; x.stop = y.pop; z.stop < x.stop
    have hze_xs : z.stop < x.stop := h1.1 ▸ h2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_contains hss (ltr hz hze_xs) hze_xs]; simp [List.mem_cons]
    · rw [cl_startedBy hss hze_xs]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
      · rw [cl_after hss hse]; simp [List.mem_cons]
      · rw [cl_metBy hss hse]; simp [List.mem_cons]
      · rw [cl_overlappedBy hss hse hze_xs]; simp [List.mem_cons]
  | .finishes, .startedBy =>
    -- h2: y.start = z.start ∧ z.stop < y.pop; z.start < x.start; z.stop < x.stop
    have hzs_xa : z.start < x.start := h2.1 ▸ h1.2
    have hze_xs : z.stop < x.stop := h1.1 ▸ h2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
    · rw [cl_after hzs_xa hse]; simp [List.mem_cons]
    · rw [cl_metBy hzs_xa hse]; simp [List.mem_cons]
    · rw [cl_overlappedBy hzs_xa hse hze_xs]; simp [List.mem_cons]
  | .finishes, .overlappedBy =>
    -- h2: z.start < y.start ∧ y.start < z.pop ∧ z.pop < y.pop; y.start < x.start; x.stop = y.pop
    -- z.pop < x.stop; z.start < y.start < x.start
    have hzs_xa : z.start < x.start := ltr h2.1 h1.2
    have hze_xs : z.stop < x.stop := h1.1 ▸ h2.2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
    · rw [cl_after hzs_xa hse]; simp [List.mem_cons]
    · rw [cl_metBy hzs_xa hse]; simp [List.mem_cons]
    · rw [cl_overlappedBy hzs_xa hse hze_xs]; simp [List.mem_cons]
  | .finishes, .metBy =>
    have hze_xa : z.stop < x.start := h2 ▸ h1.2
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])
  | .finishes, .after =>
    have hze_xa : z.stop < x.start := ltr h2 h1.2
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])

  -- ════════ r1 = equals: h1 : x.start = y.start ∧ x.stop = y.stop ════════
  | .equals, .before =>
    have hse : x.stop < z.start := h1.2 ▸ h2
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .equals, .meets =>
    have heq : x.stop = z.start := h1.2.trans h2
    simp only [compose]; exact mem_cl (cl_meets (heq ▸ hx) heq) (by simp [List.mem_cons])
  | .equals, .overlaps =>
    simp only [compose]; exact mem_cl (cl_overlaps (h1.1 ▸ h2.1) (h1.2 ▸ h2.2.1) (h1.2 ▸ h2.2.2)) (by simp [List.mem_cons])
  | .equals, .starts =>
    simp only [compose]; exact mem_cl (cl_starts (h1.1 ▸ h2.1) (h1.2 ▸ h2.2)) (by simp [List.mem_cons])
  | .equals, .during =>
    -- h1: x.start = y.start ∧ x.stop = y.stop; h2: z.start < y.start ∧ y.stop < z.stop
    -- cl_during needs: z.start < x.start, x.start < z.stop, x.stop < z.stop
    have hzs_xa : z.start < x.start := h1.1 ▸ h2.1
    have hxs : x.stop < z.stop := h1.2 ▸ h2.2
    simp only [compose]; exact mem_cl (cl_during hzs_xa (ltr hx hxs) hxs) (by simp [List.mem_cons])
  | .equals, .finishes =>
    -- h2: y.stop = z.stop ∧ z.start < y.start; cl_finishes needs: z.start < x.start, x.start < z.stop, x.stop = z.stop
    have hzs_xa : z.start < x.start := h1.1 ▸ h2.2
    have heq : x.stop = z.stop := h1.2.trans h2.1
    simp only [compose]; exact mem_cl (cl_finishes hzs_xa (heq ▸ hx) heq) (by simp [List.mem_cons])
  | .equals, .equals =>
    simp only [compose]; exact mem_cl (cl_equals (h1.1 ▸ h2.1) (h1.2 ▸ h2.2)) (by simp [List.mem_cons])
  | .equals, .finishedBy =>
    simp only [compose]; exact mem_cl (cl_finishedBy (h1.1 ▸ h2.2) (h1.2 ▸ h2.1 ▸ hz) (h1.2 ▸ h2.1)) (by simp [List.mem_cons])
  | .equals, .contains =>
    simp only [compose]; exact mem_cl (cl_contains (h1.1 ▸ h2.1) (h1.2 ▸ ltr hz h2.2) (h1.2 ▸ h2.2)) (by simp [List.mem_cons])
  | .equals, .startedBy =>
    simp only [compose]; exact mem_cl (cl_startedBy (h1.1 ▸ h2.1) (h1.2 ▸ h2.2)) (by simp [List.mem_cons])
  | .equals, .overlappedBy =>
    -- h2: z.start < y.start ∧ y.start < z.stop ∧ z.stop < y.stop
    -- cl_overlappedBy needs: z.start < x.start, x.start < z.stop, z.stop < x.stop
    have hzs_xa : z.start < x.start := h1.1 ▸ h2.1
    have hxa_ze : x.start < z.stop := h1.1 ▸ h2.2.1
    have hze_xs : z.stop < x.stop := h1.2 ▸ h2.2.2
    simp only [compose]; exact mem_cl (cl_overlappedBy hzs_xa hxa_ze hze_xs) (by simp [List.mem_cons])
  | .equals, .metBy =>
    have heq : z.stop = x.start := h2.trans h1.1.symm
    simp only [compose]; exact mem_cl (cl_metBy (heq ▸ hz) heq) (by simp [List.mem_cons])
  | .equals, .after =>
    have hze_xa : z.stop < x.start := h1.1 ▸ h2
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])

  -- ════════ r1 = finishedBy: h1 : x.stop = y.pop ∧ x.start < y.start ════════
  | .finishedBy, .before =>
    -- h2: y.pop < z.start; x.stop = y.pop < z.start
    have hse : x.stop < z.start := h1.1 ▸ h2
    simp only [compose]; exact mem_cl (cl_before (ltr hx hse) hse) (by simp [List.mem_cons])
  | .finishedBy, .meets =>
    -- h2: y.pop = z.start; x.stop = y.pop = z.start
    have heq : x.stop = z.start := h1.1.trans h2
    simp only [compose]; exact mem_cl (cl_meets (heq ▸ hx) heq) (by simp [List.mem_cons])
  | .finishedBy, .overlaps =>
    -- h2: y.start < z.start ∧ z.start < y.pop ∧ y.pop < z.stop
    -- x.stop = y.pop; z.start < x.stop; x.start < y.start < z.start
    have hss : x.start < z.start := ltr h1.2 h2.1
    have hzs_xe : z.start < x.stop := h1.1 ▸ h2.2.1
    have hxs : x.stop < z.stop := h1.1 ▸ h2.2.2
    simp only [compose]; exact mem_cl (cl_overlaps hss hzs_xe hxs) (by simp [List.mem_cons])
  | .finishedBy, .starts =>
    -- h2: y.start = z.start ∧ y.pop < z.stop; x.start < y.start = z.start; x.stop = y.pop < z.stop
    have hss : x.start < z.start := h2.1 ▸ h1.2
    have hzs_xe : z.start < x.stop := h1.1 ▸ (h2.1 ▸ hy)
    simp only [compose]; exact mem_cl (cl_overlaps hss hzs_xe (h1.1 ▸ h2.2)) (by simp [List.mem_cons])
  | .finishedBy, .during =>
    -- h2: z.start < y.start ∧ y.pop < z.stop; x.start < y.start; x.stop = y.pop
    -- x.stop = y.pop > z.start (since z.start < y.start ≤ y.pop = x.stop)
    have hxs : x.stop < z.stop := h1.1 ▸ h2.2
    have hzs_xe : z.start < x.stop := h1.1 ▸ ltr h2.1 hy
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_overlaps hss hzs_xe hxs]; simp [List.mem_cons]
    · rw [cl_starts hss hxs]; simp [List.mem_cons]
    · rw [cl_during hss (ltr hx hxs) hxs]; simp [List.mem_cons]
  | .finishedBy, .finishes =>
    -- h2: y.pop = z.stop ∧ z.start < y.start; x.stop = y.pop = z.stop; x.start vs z.start
    have heq : x.stop = z.stop := h1.1.trans h2.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_finishedBy hss (heq ▸ hz) heq]; simp [List.mem_cons]
    · rw [cl_equals hss heq]; simp [List.mem_cons]
    · rw [cl_finishes hss (heq ▸ hx) heq]; simp [List.mem_cons]
  | .finishedBy, .equals =>
    -- h2: y.start = z.start ∧ y.pop = z.stop; x.stop = z.stop; x.start < y.start = z.start
    have hss : x.start < z.start := h2.1 ▸ h1.2
    have heq : x.stop = z.stop := h1.1.trans h2.2
    simp only [compose]; exact mem_cl (cl_finishedBy hss (heq ▸ hz) heq) (by simp [List.mem_cons])
  | .finishedBy, .finishedBy =>
    -- h2: y.pop = z.stop ∧ y.start < z.start; x.start < y.start < z.start; x.stop = z.stop
    have hss : x.start < z.start := ltr h1.2 h2.2
    have heq : x.stop = z.stop := h1.1.trans h2.1
    simp only [compose]; exact mem_cl (cl_finishedBy hss (heq ▸ hz) heq) (by simp [List.mem_cons])
  | .finishedBy, .contains =>
    -- h2: y.start < z.start ∧ z.stop < y.pop; x.start < y.start < z.start; x.stop = y.pop; z.stop < x.stop
    have hss : x.start < z.start := ltr h1.2 h2.1
    have hze_xs : z.stop < x.stop := h1.1 ▸ h2.2
    simp only [compose]; exact mem_cl (cl_contains hss (ltr hz hze_xs) hze_xs) (by simp [List.mem_cons])
  | .finishedBy, .startedBy =>
    -- h2: y.start = z.start ∧ z.stop < y.pop; x.start < y.start = z.start; x.stop = y.pop; z.stop < x.stop
    have hss : x.start < z.start := h2.1 ▸ h1.2
    have hze_xs : z.stop < x.stop := h1.1 ▸ h2.2
    simp only [compose]; exact mem_cl (cl_contains hss (ltr hz hze_xs) hze_xs) (by simp [List.mem_cons])
  | .finishedBy, .overlappedBy =>
    -- h2: z.start < y.start ∧ y.start < z.pop ∧ z.pop < y.pop; x.start < y.start; x.stop = y.pop
    -- z.pop < x.stop; x.start < y.start; x.start vs z.start
    have hze_xs : z.stop < x.stop := h1.1 ▸ h2.2.2
    have hxa_ze : x.start < z.stop := ltr h1.2 h2.2.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_contains hss (ltr hz hze_xs) hze_xs]; simp [List.mem_cons]
    · rw [cl_startedBy hss hze_xs]; simp [List.mem_cons]
    · rw [cl_overlappedBy hss hxa_ze hze_xs]; simp [List.mem_cons]
  | .finishedBy, .metBy =>
    -- h2: z.stop = y.start; x.start < y.start = z.stop; x.stop = y.pop > y.start
    -- z.stop = y.start; x.start < z.stop; z.stop < x.stop (y.start < y.pop = x.stop)
    have hxst_ze : x.start < z.stop := h2 ▸ h1.2
    have hze_xs : z.stop < x.stop := h1.1 ▸ (h2 ▸ hy)
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_contains hss (ltr hz hze_xs) hze_xs]; simp [List.mem_cons]
    · rw [cl_startedBy hss hze_xs]; simp [List.mem_cons]
    · rw [cl_overlappedBy hss hxst_ze hze_xs]; simp [List.mem_cons]
  | .finishedBy, .after =>
    -- h2: z.stop < y.start; x.start < y.start; z.stop < y.start; x.stop = y.pop > y.start
    -- z.stop < y.start ≤ x.stop. x.start < y.start. x.start vs z.start?
    have hze_xs : z.stop < x.stop := h1.1 ▸ ltr h2 hy
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_contains hss (ltr hz hze_xs) hze_xs]; simp [List.mem_cons]
    · rw [cl_startedBy hss hze_xs]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
      · rw [cl_after hss hse]; simp [List.mem_cons]
      · rw [cl_metBy hss hse]; simp [List.mem_cons]
      · rw [cl_overlappedBy hss hse hze_xs]; simp [List.mem_cons]

  -- ════════ r1 = contains: h1 : x.start < y.start ∧ y.pop < x.stop ════════
  | .contains, .before =>
    -- h2: y.stop < z.start; x.start < y.start ≤ y.stop < z.start → x.start < z.start
    have hss : x.start < z.start := ltr h1.1 (ltr hy h2)
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
    · rw [cl_before hss hse]; simp [List.mem_cons]
    · rw [cl_meets hss hse]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_overlaps hss hse hee]; simp [List.mem_cons]
      · rw [cl_finishedBy hss hse hee]; simp [List.mem_cons]
      · rw [cl_contains hss hse hee]; simp [List.mem_cons]
  | .contains, .meets =>
    -- h2: y.stop = z.start; y.stop < x.stop → z.start < x.stop; x.start < y.start ≤ y.stop = z.start → x.start < z.start
    have hzs_xe : z.start < x.stop := h2 ▸ h1.2
    have hss : x.start < z.start := ltr h1.1 (h2 ▸ hy)
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_overlaps hss hzs_xe hee]; simp [List.mem_cons]
    · rw [cl_finishedBy hss hzs_xe hee]; simp [List.mem_cons]
    · rw [cl_contains hss hzs_xe hee]; simp [List.mem_cons]

  -- ════════ r1 = contains (continued): h1 : x.start < y.start ∧ y.stop < x.stop ════════
  | .contains, .overlaps =>
    -- h2: y.start < z.start ∧ z.start < y.stop ∧ y.stop < z.stop
    -- x.start < y.start < z.start; z.start < y.stop < x.stop; so z.start < x.stop
    have hss : x.start < z.start := ltr h1.1 h2.1
    have hzs_xe : z.start < x.stop := ltr h2.2.1 h1.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_overlaps hss hzs_xe hee]; simp [List.mem_cons]
    · rw [cl_finishedBy hss hzs_xe hee]; simp [List.mem_cons]
    · rw [cl_contains hss hzs_xe hee]; simp [List.mem_cons]
  | .contains, .starts =>
    -- h2: y.start = z.start ∧ y.stop < z.stop; x.start < y.start = z.start; y.start < x.stop
    have hss : x.start < z.start := h2.1 ▸ h1.1
    have hzs_xe : z.start < x.stop := h2.1 ▸ ltr hy h1.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_overlaps hss hzs_xe hee]; simp [List.mem_cons]
    · rw [cl_finishedBy hss hzs_xe hee]; simp [List.mem_cons]
    · rw [cl_contains hss hzs_xe hee]; simp [List.mem_cons]
  | .contains, .during =>
    -- h2: z.start < y.start ∧ y.stop < z.stop; x.start < y.start; y.stop < x.stop
    -- x.start vs z.start unknown; z.start < y.start < x.stop => z.start < x.stop
    -- x.stop vs z.stop unknown
    have hzs_xe : z.start < x.stop := ltr h2.1 (ltr hy h1.2)
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_overlaps hss hzs_xe hee]; simp [List.mem_cons]
      · rw [cl_finishedBy hss hzs_xe hee]; simp [List.mem_cons]
      · rw [cl_contains hss hzs_xe hee]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_starts hss hee]; simp [List.mem_cons]
      · rw [cl_equals hss hee]; simp [List.mem_cons]
      · rw [cl_startedBy hss hee]; simp [List.mem_cons]
    · have hxa_ze : x.start < z.stop := ltr h1.1 (ltr hy h2.2)
      rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_during hss hxa_ze hee]; simp [List.mem_cons]
      · rw [cl_finishes hss hxa_ze hee]; simp [List.mem_cons]
      · rw [cl_overlappedBy hss hxa_ze hee]; simp [List.mem_cons]
  | .contains, .finishes =>
    -- h2: y.stop = z.stop ∧ z.start < y.start; y.stop < x.stop => z.stop < x.stop
    -- z.start < y.start; x.start < y.start; z.start < x.stop
    have hze_xs : z.stop < x.stop := h2.1 ▸ h1.2
    have hzs_xe : z.start < x.stop := ltr h2.2 (ltr hy h1.2)
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_contains hss hzs_xe hze_xs]; simp [List.mem_cons]
    · rw [cl_startedBy hss hze_xs]; simp [List.mem_cons]
    · rw [cl_overlappedBy hss (h2.1 ▸ ltr h1.1 hy) hze_xs]; simp [List.mem_cons]
  | .contains, .equals =>
    -- h2: y.start = z.start ∧ y.stop = z.stop; x.start < z.start; z.stop < x.stop
    have hss : x.start < z.start := h2.1 ▸ h1.1
    have hze_xs : z.stop < x.stop := h2.2 ▸ h1.2
    simp only [compose]; exact mem_cl (cl_contains hss (ltr hz hze_xs) hze_xs) (by simp [List.mem_cons])
  | .contains, .finishedBy =>
    -- h2: y.stop = z.stop ∧ y.start < z.start; x.start < y.start < z.start; y.stop = z.stop < x.stop
    have hss : x.start < z.start := ltr h1.1 h2.2
    have hze_xs : z.stop < x.stop := h2.1 ▸ h1.2
    simp only [compose]; exact mem_cl (cl_contains hss (ltr hz hze_xs) hze_xs) (by simp [List.mem_cons])
  | .contains, .contains =>
    -- h2: y.start < z.start ∧ z.stop < y.stop; x.start < y.start < z.start; z.stop < y.stop; y.stop < x.stop => z.stop < x.stop
    have hss : x.start < z.start := ltr h1.1 h2.1
    have hze_xs : z.stop < x.stop := ltr h2.2 h1.2
    simp only [compose]; exact mem_cl (cl_contains hss (ltr hz hze_xs) hze_xs) (by simp [List.mem_cons])
  | .contains, .startedBy =>
    -- h2: y.start = z.start ∧ z.stop < y.stop; x.start < y.start = z.start; z.stop < y.pop < x.stop
    have hss : x.start < z.start := h2.1 ▸ h1.1
    have hze_xs : z.stop < x.stop := ltr h2.2 h1.2
    simp only [compose]; exact mem_cl (cl_contains hss (ltr hz hze_xs) hze_xs) (by simp [List.mem_cons])
  | .contains, .overlappedBy =>
    -- h2: z.start < y.start ∧ y.start < z.stop ∧ z.stop < y.pop; y.pop < x.stop => z.stop < x.stop
    -- z.start < y.start; x.start < y.start; z.start vs x.start unknown
    have hze_xs : z.stop < x.stop := ltr h2.2.2 h1.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_contains hss (ltr hz hze_xs) hze_xs]; simp [List.mem_cons]
    · rw [cl_startedBy hss hze_xs]; simp [List.mem_cons]
    · rw [cl_overlappedBy hss (ltr h1.1 h2.2.1) hze_xs]; simp [List.mem_cons]
  | .contains, .metBy =>
    -- h2: z.stop = y.start; x.start < y.start; y.stop < x.stop
    -- z.stop = y.start; y.start < x.stop => z.stop < x.stop
    -- x.start < y.start = z.stop => x.start < z.stop
    have hze_xs : z.stop < x.stop := h2 ▸ ltr hy h1.2
    have hxst_ze : x.start < z.stop := h2 ▸ h1.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_contains hss (ltr hz hze_xs) hze_xs]; simp [List.mem_cons]
    · rw [cl_startedBy hss hze_xs]; simp [List.mem_cons]
    · rw [cl_overlappedBy hss hxst_ze hze_xs]; simp [List.mem_cons]
  | .contains, .after =>
    -- h2: z.stop < y.start; x.start < y.start; y.pop < x.stop
    -- z.stop < y.start; y.start ≤ y.pop < x.stop => z.stop < x.stop
    have hze_xs : z.stop < x.stop := ltr h2 (ltr hy h1.2)
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_contains hss (ltr hz hze_xs) hze_xs]; simp [List.mem_cons]
    · rw [cl_startedBy hss hze_xs]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
      · rw [cl_after hss hse]; simp [List.mem_cons]
      · rw [cl_metBy hss hse]; simp [List.mem_cons]
      · rw [cl_overlappedBy hss hse hze_xs]; simp [List.mem_cons]

  -- ════════ r1 = startedBy: h1 : x.start = y.start ∧ y.stop < x.stop ════════
  | .startedBy, .before =>
    -- h2: y.stop < z.start; x.start = y.start; y.stop < x.stop
    -- x.start = y.start ≤ y.stop < z.start => x.start < z.start
    have hss : x.start < z.start := h1.1 ▸ ltr hy h2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
    · rw [cl_before hss hse]; simp [List.mem_cons]
    · rw [cl_meets hss hse]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_overlaps hss hse hee]; simp [List.mem_cons]
      · rw [cl_finishedBy hss hse hee]; simp [List.mem_cons]
      · rw [cl_contains hss hse hee]; simp [List.mem_cons]
  | .startedBy, .meets =>
    -- h2: y.stop = z.start; y.stop < x.stop => z.start < x.stop; x.start = y.start ≤ y.stop = z.start
    have hzs_xe : z.start < x.stop := h2 ▸ h1.2
    have hss : x.start < z.start := h1.1 ▸ (h2 ▸ hy)
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_overlaps hss hzs_xe hee]; simp [List.mem_cons]
    · rw [cl_finishedBy hss hzs_xe hee]; simp [List.mem_cons]
    · rw [cl_contains hss hzs_xe hee]; simp [List.mem_cons]
  | .startedBy, .overlaps =>
    -- h2: y.start < z.start ∧ z.start < y.stop ∧ y.stop < z.stop
    -- x.start = y.start < z.start; z.start < y.pop < x.stop
    have hss : x.start < z.start := h1.1 ▸ h2.1
    have hzs_xe : z.start < x.stop := ltr h2.2.1 h1.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_overlaps hss hzs_xe hee]; simp [List.mem_cons]
    · rw [cl_finishedBy hss hzs_xe hee]; simp [List.mem_cons]
    · rw [cl_contains hss hzs_xe hee]; simp [List.mem_cons]
  | .startedBy, .starts =>
    -- h2: y.start = z.start ∧ y.stop < z.stop; x.start = y.start = z.start
    have hss : x.start = z.start := h1.1.trans h2.1
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_starts hss hee]; simp [List.mem_cons]
    · rw [cl_equals hss hee]; simp [List.mem_cons]
    · rw [cl_startedBy hss hee]; simp [List.mem_cons]
  | .startedBy, .during =>
    -- h2: z.start < y.start ∧ y.stop < z.stop; x.start = y.start
    -- z.start < x.start; x.start < z.stop (since x.start = y.start ≤ y.stop < z.stop)
    have hzs_xa : z.start < x.start := h1.1 ▸ h2.1
    have hxa_ze : x.start < z.stop := h1.1 ▸ ltr hy h2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_during hzs_xa hxa_ze hee]; simp [List.mem_cons]
    · rw [cl_finishes hzs_xa hxa_ze hee]; simp [List.mem_cons]
    · rw [cl_overlappedBy hzs_xa hxa_ze hee]; simp [List.mem_cons]
  | .startedBy, .finishes =>
    -- h2: y.stop = z.stop ∧ z.start < y.start; x.start = y.start; y.pop = z.stop < x.stop
    -- z.start < x.start; x.start < z.stop (x.start = y.start ≤ y.pop = z.stop)
    have hzs_xa : z.start < x.start := h1.1 ▸ h2.2
    have hze_xs : z.stop < x.stop := h2.1 ▸ h1.2
    have hxa_ze : x.start < z.stop := h1.1 ▸ (h2.1 ▸ hy)
    simp only [compose]; exact mem_cl (cl_overlappedBy hzs_xa hxa_ze hze_xs) (by simp [List.mem_cons])
  | .startedBy, .equals =>
    -- h2: y.start = z.start ∧ y.pop = z.stop; x.start = z.start; z.stop < x.stop
    have hss : x.start = z.start := h1.1.trans h2.1
    have hze_xs : z.stop < x.stop := h2.2 ▸ h1.2
    simp only [compose]; exact mem_cl (cl_startedBy hss hze_xs) (by simp [List.mem_cons])
  | .startedBy, .finishedBy =>
    -- h2: y.pop = z.stop ∧ y.start < z.start; x.start = y.start < z.start; y.pop = z.stop < x.stop? No: y.pop < x.stop => z.stop < x.stop
    have hss : x.start < z.start := h1.1 ▸ h2.2
    have hze_xs : z.stop < x.stop := h2.1 ▸ h1.2
    simp only [compose]; exact mem_cl (cl_contains hss (ltr hz hze_xs) hze_xs) (by simp [List.mem_cons])
  | .startedBy, .contains =>
    -- h2: y.start < z.start ∧ z.stop < y.pop; x.start = y.start < z.start; z.stop < y.pop < x.stop
    have hss : x.start < z.start := h1.1 ▸ h2.1
    have hze_xs : z.stop < x.stop := ltr h2.2 h1.2
    simp only [compose]; exact mem_cl (cl_contains hss (ltr hz hze_xs) hze_xs) (by simp [List.mem_cons])
  | .startedBy, .startedBy =>
    -- h2: y.start = z.start ∧ z.stop < y.pop; x.start = y.start = z.start; z.stop < y.pop < x.stop
    have hss : x.start = z.start := h1.1.trans h2.1
    have hze_xs : z.stop < x.stop := ltr h2.2 h1.2
    simp only [compose]; exact mem_cl (cl_startedBy hss hze_xs) (by simp [List.mem_cons])
  | .startedBy, .overlappedBy =>
    -- h2: z.start < y.start ∧ y.start < z.stop ∧ z.stop < y.pop; x.start = y.start; z.stop < y.pop < x.stop
    -- z.start < x.start; x.start < z.stop; z.stop < x.stop
    have hzs_xa : z.start < x.start := h1.1 ▸ h2.1
    have hxa_ze : x.start < z.stop := h1.1 ▸ h2.2.1
    have hze_xs : z.stop < x.stop := ltr h2.2.2 h1.2
    simp only [compose]; exact mem_cl (cl_overlappedBy hzs_xa hxa_ze hze_xs) (by simp [List.mem_cons])
  | .startedBy, .metBy =>
    -- h2: z.stop = y.start = x.start
    have heq : z.stop = x.start := h2.trans h1.1.symm
    simp only [compose]; exact mem_cl (cl_metBy (heq ▸ hz) heq) (by simp [List.mem_cons])
  | .startedBy, .after =>
    -- h2: z.stop < y.start = x.start
    have hze_xa : z.stop < x.start := h1.1 ▸ h2
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])

  -- ════════ r1 = overlappedBy: h1 : y.start < x.start ∧ x.start < y.stop ∧ y.stop < x.stop ════════
  | .overlappedBy, .before =>
    -- h2: y.stop < z.start; y.start < x.start; x.start < y.stop; y.stop < x.stop
    -- x.start < y.stop < z.start => x.start < z.start; y.pop < z.start; y.pop < x.stop
    have hss : x.start < z.start := ltr h1.2.1 h2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
    · rw [cl_before hss hse]; simp [List.mem_cons]
    · rw [cl_meets hss hse]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_overlaps hss hse hee]; simp [List.mem_cons]
      · rw [cl_finishedBy hss hse hee]; simp [List.mem_cons]
      · rw [cl_contains hss hse hee]; simp [List.mem_cons]
  | .overlappedBy, .meets =>
    -- h2: y.stop = z.start; y.pop < x.stop => z.start < x.stop
    -- x.start < y.pop = z.start => x.start < z.start
    have hss : x.start < z.start := h2 ▸ h1.2.1
    have hzs_xe : z.start < x.stop := h2 ▸ h1.2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_overlaps hss hzs_xe hee]; simp [List.mem_cons]
    · rw [cl_finishedBy hss hzs_xe hee]; simp [List.mem_cons]
    · rw [cl_contains hss hzs_xe hee]; simp [List.mem_cons]
  | .overlappedBy, .overlaps =>
    -- h2: y.start < z.start ∧ z.start < y.stop ∧ y.stop < z.stop
    -- y.start < x.start; x.start < y.pop; y.pop < x.stop
    -- x.start vs z.start unknown; z.start < y.pop < x.stop => z.start < x.stop
    -- x.stop vs z.stop unknown
    have hzs_xe : z.start < x.stop := ltr h2.2.1 h1.2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_overlaps hss hzs_xe hee]; simp [List.mem_cons]
      · rw [cl_finishedBy hss hzs_xe hee]; simp [List.mem_cons]
      · rw [cl_contains hss hzs_xe hee]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_starts hss hee]; simp [List.mem_cons]
      · rw [cl_equals hss hee]; simp [List.mem_cons]
      · rw [cl_startedBy hss hee]; simp [List.mem_cons]
    · have hxa_ze : x.start < z.stop := ltr h1.2.1 h2.2.2
      rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_during hss hxa_ze hee]; simp [List.mem_cons]
      · rw [cl_finishes hss hxa_ze hee]; simp [List.mem_cons]
      · rw [cl_overlappedBy hss hxa_ze hee]; simp [List.mem_cons]
  | .overlappedBy, .starts =>
    -- h2: y.start = z.start ∧ y.pop < z.stop; y.start < x.start; x.start < y.pop; y.pop < x.stop
    -- z.start < x.start; x.start < z.stop (x.start < y.pop < z.stop)
    have hzs_xa : z.start < x.start := h2.1 ▸ h1.1
    have hxa_ze : x.start < z.stop := ltr h1.2.1 h2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_during hzs_xa hxa_ze hee]; simp [List.mem_cons]
    · rw [cl_finishes hzs_xa hxa_ze hee]; simp [List.mem_cons]
    · rw [cl_overlappedBy hzs_xa hxa_ze hee]; simp [List.mem_cons]
  | .overlappedBy, .during =>
    -- h2: z.start < y.start ∧ y.pop < z.stop; y.start < x.start; x.start < y.pop; y.pop < x.stop
    -- z.start < y.start < x.start; x.start < y.pop < z.stop; x.stop vs z.stop?
    have hzs_xa : z.start < x.start := ltr h2.1 h1.1
    have hxa_ze : x.start < z.stop := ltr h1.2.1 h2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_during hzs_xa hxa_ze hee]; simp [List.mem_cons]
    · rw [cl_finishes hzs_xa hxa_ze hee]; simp [List.mem_cons]
    · rw [cl_overlappedBy hzs_xa hxa_ze hee]; simp [List.mem_cons]
  | .overlappedBy, .finishes =>
    -- h2: y.pop = z.stop ∧ z.start < y.start; y.start < x.start; y.pop < x.stop
    -- z.start < y.start < x.start; z.stop = y.pop < x.stop; x.start < y.pop = z.stop
    have hzs_xa : z.start < x.start := ltr h2.2 h1.1
    have hxa_ze : x.start < z.stop := h2.1 ▸ h1.2.1
    have hze_xs : z.stop < x.stop := h2.1 ▸ h1.2.2
    simp only [compose]; exact mem_cl (cl_overlappedBy hzs_xa hxa_ze hze_xs) (by simp [List.mem_cons])
  | .overlappedBy, .equals =>
    -- h2: y.start = z.start ∧ y.pop = z.stop; x.start > y.start = z.start; x.start < y.pop = z.stop; y.pop < x.stop
    have hzs_xa : z.start < x.start := h2.1 ▸ h1.1
    have hxa_ze : x.start < z.stop := h2.2 ▸ h1.2.1
    have hze_xs : z.stop < x.stop := h2.2 ▸ h1.2.2
    simp only [compose]; exact mem_cl (cl_overlappedBy hzs_xa hxa_ze hze_xs) (by simp [List.mem_cons])
  | .overlappedBy, .finishedBy =>
    -- h2: y.pop = z.stop ∧ y.start < z.start; y.start < x.start; x.start < y.pop; y.pop < x.stop
    -- y.start < z.start; x.start vs z.start unknown; z.stop = y.pop < x.stop; z.start < y.pop = z.stop is NOT necessarily true
    -- But z.start < x.stop (since z.start ≤ y.pop < x.stop... actually z.start could be > y.pop)
    -- Actually h2.2 says y.start < z.start, and h2.1 says y.pop = z.stop. We need z.start < x.stop.
    -- We know z.stop = y.pop < x.stop. And z.start < z.stop (hz). So z.start < x.stop.
    have hze_xs : z.stop < x.stop := h2.1 ▸ h1.2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_contains hss (ltr hz hze_xs) hze_xs]; simp [List.mem_cons]
    · rw [cl_startedBy hss hze_xs]; simp [List.mem_cons]
    · rw [cl_overlappedBy hss (h2.1 ▸ h1.2.1) hze_xs]; simp [List.mem_cons]
  | .overlappedBy, .contains =>
    -- h2: y.start < z.start ∧ z.stop < y.pop; y.start < x.start; y.pop < x.stop
    -- z.stop < y.pop < x.stop; y.start < z.start; y.start < x.start; x.start vs z.start?
    have hze_xs : z.stop < x.stop := ltr h2.2 h1.2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rw [cl_contains hss (ltr hz hze_xs) hze_xs]; simp [List.mem_cons]
    · rw [cl_startedBy hss hze_xs]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
      · rw [cl_after hss hse]; simp [List.mem_cons]
      · rw [cl_metBy hss hse]; simp [List.mem_cons]
      · rw [cl_overlappedBy hss hse hze_xs]; simp [List.mem_cons]
  | .overlappedBy, .startedBy =>
    -- h2: y.start = z.start ∧ z.stop < y.pop; y.start < x.start; y.pop < x.stop
    -- z.start < x.start; z.stop < y.pop < x.stop
    have hzs_xa : z.start < x.start := h2.1 ▸ h1.1
    have hze_xs : z.stop < x.stop := ltr h2.2 h1.2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
    · rw [cl_after hzs_xa hse]; simp [List.mem_cons]
    · rw [cl_metBy hzs_xa hse]; simp [List.mem_cons]
    · rw [cl_overlappedBy hzs_xa hse hze_xs]; simp [List.mem_cons]
  | .overlappedBy, .overlappedBy =>
    -- h2: z.start < y.start ∧ y.start < z.stop ∧ z.stop < y.pop; y.start < x.start; y.pop < x.stop
    -- z.start < y.start < x.start; z.stop < y.pop < x.stop; z.stop vs x.start?
    have hzs_xa : z.start < x.start := ltr h2.1 h1.1
    have hze_xs : z.stop < x.stop := ltr h2.2.2 h1.2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
    · rw [cl_after hzs_xa hse]; simp [List.mem_cons]
    · rw [cl_metBy hzs_xa hse]; simp [List.mem_cons]
    · rw [cl_overlappedBy hzs_xa hse hze_xs]; simp [List.mem_cons]
  | .overlappedBy, .metBy =>
    -- h2: z.stop = y.start; y.start < x.start => z.stop < x.start
    have hze_xa : z.stop < x.start := h2 ▸ h1.1
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])
  | .overlappedBy, .after =>
    -- h2: z.stop < y.start; y.start < x.start => z.stop < x.start
    have hze_xa : z.stop < x.start := ltr h2 h1.1
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])

  -- ════════ r1 = metBy: h1 : y.stop = x.start ════════
  | .metBy, .before =>
    -- h2: y.stop < z.start; y.stop = x.start => x.start < z.start? No, y.stop < z.start and y.pop = x.start? Wait.
    -- h1: y.stop = x.start; h2: y.stop < z.start => x.start < z.start? No: y.stop < z.start, y.pop = x.start but that's metBy: y.stop = x.start
    -- So x.start = y.stop. x.start < z.start? We have y.stop < z.start, and y.stop = x.start, so... wait no.
    -- metBy: h1 : y.stop = x.start. So x.start = y.stop.
    -- h2: y.stop < z.start. Wait, h2 for before is y.stop < z.start? No, for r2 = before, h2 is holds before y z = y.stop < z.start.
    -- Actually no. r2.holds y z. before.holds y z = y.stop < z.start. Yes.
    -- So x.start = y.stop; y.stop < z.start => x.start < z.start? Hmm, x.start = y.stop and y.stop < z.start... but these are equal so x.start < z.start? No, we need x.start = y.stop and y.stop < z.start. Since x.start = y.stop, we get x.start = y.stop < z.start? But = and < don't compose that way in Lean directly.
    -- Actually they do: h1 says y.stop = x.start, so x.start = y.stop. Then y.stop < z.start. We need x.start < z.start = h1 ▸ h2 should work.
    -- h1 : y.stop = x.start. h2 : y.stop < z.start. We want x.start < z.start.
    -- h1.symm ▸ h2 rewrites y.stop to x.start? No, h1 : y.stop = x.start, so h1 ▸ h2 replaces x.start with y.stop... that's wrong direction.
    -- We need h1.symm : x.start = y.stop, then h1.symm ▸ h2 doesn't help either.
    -- Let's just use: have : x.start < z.start := h1 ▸ h2... hmm.
    -- Actually in Lean, ▸ rewrites the GOAL. For have, we should use trans or direct rewriting.
    -- h1 : y.stop = x.start means y.stop = x.start
    -- So x.start = y.stop is h1.symm
    -- h2 : y.stop < z.start
    -- We want: x.start < z.start. Since x.start = y.stop (h1.symm), rewrite: y.stop < z.start = h2. Done.
    -- In Lean: h1.symm ▸ h2 rewrites x.start to y.stop in the goal? No. For terms:
    -- Let me use: have hss : x.start < z.start := h1.symm ▸ h2
    -- Hmm, ▸ works on goals not terms. Better: h1 ▸ h2 would replace x.start with y.stop in h2... but h2 doesn't mention x.start.
    -- I think the cleanest way is: have hse : x.stop < z.start := ltr hx h2... wait no.
    -- OK let me just think about what we know simply:
    -- x.start = y.stop (from h1: y.stop = x.start)
    -- y.stop < z.start (h2, since r2 = before means y.stop < z.start)
    -- So x.start < z.start? No! x.start = y.stop and y.stop < z.start, but the issue is y.stop here is y's stop and x.start is a different field.
    -- They ARE equal by h1. In the `holds` unfolding, h1 : y.stop = x.start (metBy.holds x y = y.stop = x.start).
    -- So: y.stop = x.start. y.stop < z.start. Therefore x.start < z.start.
    -- In Lean: show x.start < z.start. rw [← h1]. exact h2. Or: h1 ▸ ... no.
    -- Simplest: h1 ▸ h2 won't work. But h1.symm ▸ h2 won't work either since ▸ is for goals.
    -- Let me just use the pattern from the existing code. Looking at meets row: h1 : x.stop = y.start
    -- For meets+before: have hse : x.stop < z.start := h1 ▸ ltr hy h2
    -- So for metBy (h1 : y.stop = x.start), before (h2 : y.stop < z.start):
    -- We want x.start < z.start. Since y.stop = x.start: x.start = y.stop.
    -- We can build: y.start ≤ y.stop = x.start, and y.stop < z.start...
    -- OK simplest: x.start = y.stop, and y.stop < z.start means x.start < z.start.
    -- In Lean terms, we can write something like:
    -- have hss : x.start < z.start := by rw [← h1]; exact h2
    -- But looking at existing patterns, they prefer rewrites. Let me look at meets.metBy:
    -- | .meets, .metBy => have heq : x.stop = z.stop := h1.trans h2.symm
    -- OK so trans works. For metBy.before: h1 : y.stop = x.start, and h2 : y.stop < z.start.
    -- We want x.start < z.start. We know y.stop = x.start, y.stop < z.start.
    -- So x.start = y.stop (h1.symm), and y.stop < z.start (h2).
    -- We can chain: the existing file uses h1 ▸ for equalities in the h1 hypotheses.
    -- Actually looking at contains.before: have hss : x.start < z.start := ltr h1.1 (ltr hy h2)
    -- For metBy.before: we need x.start < z.start. We have y.stop = x.start (h1). And y.stop < z.start (h2).
    -- If we had x.start = y.stop we could use h1.symm ▸ ... but that's for goals. For terms we'd need Eq.subst or similar.
    -- Actually the simplest way in Lean 4: h1 ▸ h2 substitutes in the type... no. Let me just use:
    -- have := h1 ▸ h2  -- this won't substitute the right thing
    -- OK, I think the simplest approach is what the existing code uses for .meets cases:
    -- For meets, h1 : x.stop = y.start. E.g. | .meets, .before => have hse : x.stop < z.start := h1 ▸ ltr hy h2
    -- Wait that doesn't make sense either. Let me re-check: h1 : x.stop = y.start, h2 : y.stop < z.start (before.holds y z).
    -- have hse : x.stop < z.start := h1 ▸ ltr hy h2
    -- ltr hy h2 = lt_trans (y.start < y.stop) (y.stop < z.start) = y.start < z.start
    -- h1 ▸ (y.start < z.start): h1 : x.stop = y.start, so ▸ rewrites y.start to x.stop? Or x.stop to y.start?
    -- In Lean 4, h ▸ e where h : a = b rewrites b to a in e's type. So h1 ▸ (y.start < z.start) would rewrite y.start to x.stop, giving x.stop < z.start. That's correct!
    -- So for metBy (h1 : y.stop = x.start), h1 ▸ h2 would rewrite x.start to y.stop in h2's type. But h2 : y.stop < z.start doesn't mention x.start. So that won't work.
    -- We need x.start < z.start. h1 : y.stop = x.start. h1 ▸ would rewrite x.start to y.stop.
    -- We want to go the other direction: rewrite y.stop to x.start in h2. That's h1.symm ▸ h2? No, h1.symm : x.start = y.stop, and h1.symm ▸ h2 would rewrite y.stop to x.start in h2: y.stop < z.start → x.start < z.start. Yes!
    -- Wait, I need to double check: h : a = b, then h ▸ e rewrites b → a? Or a → b?
    -- In Lean 4: `h ▸ e` where `h : a = b` replaces `b` with `a` in the expected type. When used as a term, it means: given `e : P b`, produce `P a`. No wait, it's the opposite for terms vs tactics.
    -- Actually `Eq.subst h` takes `P a → P b` when `h : a = b`. And `▸` in tactic mode rewrites the goal.
    -- For term-mode, we'd use: `h1.symm ▸ h2` but that's tactic notation... in term mode it doesn't work.
    -- OK let me just look at how the file handles the symmetric case. In the meets row:
    -- | .meets, .after => have hze_xs : z.stop < x.stop := h1 ▸ h2
    -- h1 : x.stop = y.start, h2 : z.stop < y.start (after.holds y z). h1 ▸ h2 would rewrite... this is in a `have` declaration. In Lean 4, `have foo := h1 ▸ h2` uses `▸` in tactic mode within the `by` block? No, this isn't in a `by` block.
    -- Actually wait, `h ▸ e` in TERM mode: when h : a = b and e : P a, then h ▸ e : P b. (It's Eq.mpr direction.)
    -- So h1 : x.stop = y.start, h2 : z.stop < y.start. h1 ▸ h2 would need h2 : P x.stop and produce P y.start? No.
    -- Let me check: h ▸ in term mode uses `Eq.mpr`. `Eq.mpr (h : a = b) : b → a` for types... no. `Eq.mpr` takes a proof that `α = β` and converts `β` to `α`.
    -- I think for propositions: if `h : a = b` then `h ▸ e` where `e : P a` gives `P b`? Or `e : P b` gives `P a`?
    -- In Lean 4 term mode, `▸` uses `Eq.mpr` (or similar). The convention: `h ▸ e` takes `e` of the "old" type and gives the "new" type after rewriting.
    -- Let me just try the approach that the file already uses. In meets row:
    -- `have hze_xs : z.stop < x.stop := h1 ▸ h2` where h1 : x.stop = y.start, h2 : z.stop < y.start.
    -- This should mean: the explicit type annotation `z.stop < x.stop` is the target. h1 ▸ h2 rewrites: using h1 (x.stop = y.start), replace x.stop with y.start in the goal, getting z.stop < y.start, which is h2.
    -- YES! That's how it works. `h ▸ e` with explicit type annotation: `h : a = b`, target `P a`, rewrite to `P b`, prove with `e : P b`. So it replaces `a` with `b`.
    -- So for metBy: h1 : y.stop = x.start. Target: x.start < z.start. h1 ▸ h2 would replace y.stop with x.start... but the target has x.start, not y.stop.
    -- We want to replace x.start with y.stop: need h1.symm ▸ h2? h1.symm : x.start = y.stop. Then h1.symm ▸ h2 with target x.start < z.start: replaces x.start with y.stop, getting y.stop < z.start = h2. Yes!
    -- WAIT. Actually let me re-examine. If h : a = b, then `(h ▸ e : P a)` means we need `e : P b`. The ▸ rewrites the TARGET, replacing `a` with `b`, then expects proof of `P b`.
    -- No that's wrong too. Let me just look at the Lean source/docs. Actually let me just follow the existing pattern.
    -- `have hse : x.stop < z.start := h1 ▸ ltr hy h2` where h1 : x.stop = y.start.
    -- ltr hy h2 : y.start < z.start. Target: x.stop < z.start. h1 ▸ (y.start < z.start): h1 says x.stop = y.start, so replace x.stop with y.start in target → y.start < z.start, then need proof of y.start < z.start, which is ltr hy h2.
    -- OK so `h : a = b, target P a, h ▸ e` expects `e : P b`. I.e., replaces `a` with `b` in target.
    -- So for metBy: h1 : y.stop = x.start. Target: x.start < z.start. There's no y.stop in the target. So h1 ▸ won't help directly.
    -- But h1.symm : x.start = y.stop. Target: x.start < z.start. h1.symm ▸ e expects e : y.stop < z.start = h2. Yes!
    have hss : x.start < z.start := h1 ▸ h2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
    · rw [cl_before hss hse]; simp [List.mem_cons]
    · rw [cl_meets hss hse]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_overlaps hss hse hee]; simp [List.mem_cons]
      · rw [cl_finishedBy hss hse hee]; simp [List.mem_cons]
      · rw [cl_contains hss hse hee]; simp [List.mem_cons]
  | .metBy, .meets =>
    -- h1 : y.stop = x.start; h2 : y.stop = z.start
    -- x.start = y.stop = z.start? No: h1 says y.stop = x.start, h2 says y.stop = z.start.
    -- So x.start = y.stop and z.start = y.stop, hence x.start = z.start.
    have hss : x.start = z.start := h1.symm.trans h2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_starts hss hee]; simp [List.mem_cons]
    · rw [cl_equals hss hee]; simp [List.mem_cons]
    · rw [cl_startedBy hss hee]; simp [List.mem_cons]
  | .metBy, .overlaps =>
    -- h1 : y.stop = x.start; h2 : y.start < z.start ∧ z.start < y.stop ∧ y.stop < z.stop
    -- z.start < y.stop = x.start => z.start < x.start; x.start = y.stop < z.stop
    have hzs_xa : z.start < x.start := h1.symm ▸ h2.2.1
    have hxa_ze : x.start < z.stop := h1.symm ▸ h2.2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_during hzs_xa hxa_ze hee]; simp [List.mem_cons]
    · rw [cl_finishes hzs_xa hxa_ze hee]; simp [List.mem_cons]
    · rw [cl_overlappedBy hzs_xa hxa_ze hee]; simp [List.mem_cons]
  | .metBy, .starts =>
    -- h1 : y.stop = x.start; h2 : y.start = z.start ∧ y.stop < z.stop
    -- z.start = y.start < y.stop = x.start => z.start < x.start; x.start = y.stop < z.stop
    have hzs_xa : z.start < x.start := h1.symm ▸ (h2.1 ▸ hy)
    have hxa_ze : x.start < z.stop := h1.symm ▸ h2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_during hzs_xa hxa_ze hee]; simp [List.mem_cons]
    · rw [cl_finishes hzs_xa hxa_ze hee]; simp [List.mem_cons]
    · rw [cl_overlappedBy hzs_xa hxa_ze hee]; simp [List.mem_cons]
  | .metBy, .during =>
    -- h1 : y.stop = x.start; h2 : z.start < y.start ∧ y.stop < z.stop
    -- z.start < y.start ≤ y.stop = x.start => z.start < x.start
    -- x.start = y.stop < z.stop
    have hzs_xa : z.start < x.start := h1.symm ▸ ltr h2.1 hy
    have hxa_ze : x.start < z.stop := h1.symm ▸ h2.2
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
    · rw [cl_during hzs_xa hxa_ze hee]; simp [List.mem_cons]
    · rw [cl_finishes hzs_xa hxa_ze hee]; simp [List.mem_cons]
    · rw [cl_overlappedBy hzs_xa hxa_ze hee]; simp [List.mem_cons]
  | .metBy, .finishes =>
    -- h1 : y.stop = x.start; h2 : y.stop = z.stop ∧ z.start < y.start
    -- z.stop = y.stop = x.start
    have heq : z.stop = x.start := h2.1.symm.trans h1
    have hzs_xa : z.start < x.start := h1 ▸ ltr h2.2 hy
    simp only [compose]; exact mem_cl (cl_metBy hzs_xa heq) (by simp [List.mem_cons])
  | .metBy, .equals =>
    -- h1 : y.stop = x.start; h2 : y.start = z.start ∧ y.stop = z.stop
    -- z.stop = y.stop = x.start; z.start = y.start < y.stop = x.start
    have heq : z.stop = x.start := h2.2.symm.trans h1
    have hya_xa : y.start < x.start := h1 ▸ hy
    have hzs_xa : z.start < x.start := h2.1.symm ▸ hya_xa
    simp only [compose]; exact mem_cl (cl_metBy hzs_xa heq) (by simp [List.mem_cons])
  | .metBy, .finishedBy =>
    -- h1 : y.stop = x.start; h2 : y.stop = z.stop ∧ y.start < z.start
    -- z.stop = y.stop → z.stop = x.start
    have heq : z.stop = x.start := h2.1.symm.trans h1
    have hzs_xa : z.start < x.start := heq ▸ hz
    simp only [compose]; exact mem_cl (cl_metBy hzs_xa heq) (by simp [List.mem_cons])
  | .metBy, .contains =>
    -- h1 : y.stop = x.start; h2 : y.start < z.start ∧ z.stop < y.pop
    -- contains.holds y z = y.start < z.start ∧ z.stop < y.stop.
    -- z.stop < y.stop = x.start => z.stop < x.start
    have hze_xa : z.stop < x.start := h1.symm ▸ h2.2
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])
  | .metBy, .startedBy =>
    -- h1 : y.stop = x.start; h2 : y.start = z.start ∧ z.stop < y.pop
    -- startedBy.holds y z = y.start = z.start ∧ z.stop < y.stop.
    -- z.stop < y.stop = x.start => z.stop < x.start
    have hze_xa : z.stop < x.start := h1.symm ▸ h2.2
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])
  | .metBy, .overlappedBy =>
    -- h1 : y.stop = x.start; h2 : z.start < y.start ∧ y.start < z.stop ∧ z.stop < y.pop
    -- overlappedBy.holds y z = z.start < y.start ∧ y.start < z.stop ∧ z.stop < y.stop.
    -- z.stop < y.stop = x.start => z.stop < x.start
    have hze_xa : z.stop < x.start := h1.symm ▸ h2.2.2
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])
  | .metBy, .metBy =>
    -- h1 : y.stop = x.start; h2 : z.stop = y.start
    -- z.stop = y.start < y.stop = x.start => z.stop < x.start
    have hze_xa : z.stop < x.start := h1.symm ▸ (h2 ▸ hy)
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])
  | .metBy, .after =>
    -- h1 : y.stop = x.start; h2 : z.stop < y.start
    -- z.stop < y.start ≤ y.stop = x.start => z.stop < x.start
    have hze_xa : z.stop < x.start := h1.symm ▸ ltr h2 hy
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])

  -- ════════ r1 = after: h1 : y.stop < x.start ════════
  | .after, .before =>
    -- h2 : y.stop < z.start; h1 : y.stop < x.start
    -- x.start vs z.start unknown; all 13 possible
    simp only [compose]
    rcases TimePoint.lt_trichotomy x.start z.start with hss | hss | hss
    · rcases TimePoint.lt_trichotomy x.stop z.start with hse | hse | hse
      · rw [cl_before hss hse]; simp [List.mem_cons]
      · rw [cl_meets hss hse]; simp [List.mem_cons]
      · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
        · rw [cl_overlaps hss hse hee]; simp [List.mem_cons]
        · rw [cl_finishedBy hss hse hee]; simp [List.mem_cons]
        · rw [cl_contains hss hse hee]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_starts hss hee]; simp [List.mem_cons]
      · rw [cl_equals hss hee]; simp [List.mem_cons]
      · rw [cl_startedBy hss hee]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
      · rw [cl_after hss hse]; simp [List.mem_cons]
      · rw [cl_metBy hss hse]; simp [List.mem_cons]
      · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
        · rw [cl_during hss hse hee]; simp [List.mem_cons]
        · rw [cl_finishes hss hse hee]; simp [List.mem_cons]
        · rw [cl_overlappedBy hss hse hee]; simp [List.mem_cons]
  | .after, .meets =>
    -- h1 : y.stop < x.start; h2 : y.stop = z.start
    -- z.start = y.stop < x.start; z.start < x.start
    have hzs_xa : z.start < x.start := h2 ▸ h1
    simp only [compose]
    rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
    · rw [cl_after hzs_xa hse]; simp [List.mem_cons]
    · rw [cl_metBy hzs_xa hse]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_during hzs_xa hse hee]; simp [List.mem_cons]
      · rw [cl_finishes hzs_xa hse hee]; simp [List.mem_cons]
      · rw [cl_overlappedBy hzs_xa hse hee]; simp [List.mem_cons]
  | .after, .overlaps =>
    -- h1 : y.stop < x.start; h2 : y.start < z.start ∧ z.start < y.stop ∧ y.stop < z.stop
    -- z.start < y.stop < x.start; x.start < z.stop? y.stop < z.stop and y.pop < x.start...
    -- Actually x.start vs z.stop is unknown. But z.start < x.start.
    have hzs_xa : z.start < x.start := ltr h2.2.1 h1
    -- x.start vs z.stop: y.pop < x.start and y.pop < z.stop, but no ordering between x.start and z.stop
    simp only [compose]
    rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
    · rw [cl_after hzs_xa hse]; simp [List.mem_cons]
    · rw [cl_metBy hzs_xa hse]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_during hzs_xa hse hee]; simp [List.mem_cons]
      · rw [cl_finishes hzs_xa hse hee]; simp [List.mem_cons]
      · rw [cl_overlappedBy hzs_xa hse hee]; simp [List.mem_cons]
  | .after, .starts =>
    -- h1 : y.stop < x.start; h2 : y.start = z.start ∧ y.stop < z.stop
    -- z.start = y.start ≤ y.stop < x.start => z.start < x.start
    -- x.start vs z.stop: y.stop < both, unknown
    have hzs_xa : z.start < x.start := ltr (h2.1 ▸ hy) h1
    simp only [compose]
    rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
    · rw [cl_after hzs_xa hse]; simp [List.mem_cons]
    · rw [cl_metBy hzs_xa hse]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_during hzs_xa hse hee]; simp [List.mem_cons]
      · rw [cl_finishes hzs_xa hse hee]; simp [List.mem_cons]
      · rw [cl_overlappedBy hzs_xa hse hee]; simp [List.mem_cons]
  | .after, .during =>
    -- h1 : y.stop < x.start; h2 : z.start < y.start ∧ y.stop < z.stop
    -- z.start < y.start ≤ y.stop < x.start => z.start < x.start
    have hzs_xa : z.start < x.start := ltr h2.1 (ltr hy h1)
    simp only [compose]
    rcases TimePoint.lt_trichotomy z.stop x.start with hse | hse | hse
    · rw [cl_after hzs_xa hse]; simp [List.mem_cons]
    · rw [cl_metBy hzs_xa hse]; simp [List.mem_cons]
    · rcases TimePoint.lt_trichotomy x.stop z.stop with hee | hee | hee
      · rw [cl_during hzs_xa hse hee]; simp [List.mem_cons]
      · rw [cl_finishes hzs_xa hse hee]; simp [List.mem_cons]
      · rw [cl_overlappedBy hzs_xa hse hee]; simp [List.mem_cons]
  | .after, .finishes =>
    -- h1 : y.stop < x.start; h2 : y.stop = z.stop ∧ z.start < y.start
    -- z.stop = y.stop < x.start => z.stop < x.start
    have hze_xa : z.stop < x.start := h2.1 ▸ h1
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])
  | .after, .equals =>
    -- h1 : y.stop < x.start; h2 : y.start = z.start ∧ y.stop = z.stop
    -- z.stop = y.stop < x.start
    have hze_xa : z.stop < x.start := h2.2 ▸ h1
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])
  | .after, .finishedBy =>
    -- h1 : y.stop < x.start; h2 : y.stop = z.stop ∧ y.start < z.start
    -- z.stop = y.stop < x.start
    have hze_xa : z.stop < x.start := h2.1 ▸ h1
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])
  | .after, .contains =>
    -- h1 : y.stop < x.start; h2 : y.start < z.start ∧ z.stop < y.stop
    -- z.stop < y.stop < x.start
    have hze_xa : z.stop < x.start := ltr h2.2 h1
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])
  | .after, .startedBy =>
    -- h1 : y.stop < x.start; h2 : y.start = z.start ∧ z.stop < y.stop
    -- z.stop < y.stop < x.start
    have hze_xa : z.stop < x.start := ltr h2.2 h1
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])
  | .after, .overlappedBy =>
    -- h1 : y.stop < x.start; h2 : z.start < y.start ∧ y.start < z.stop ∧ z.stop < y.stop
    -- z.stop < y.stop < x.start
    have hze_xa : z.stop < x.start := ltr h2.2.2 h1
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])
  | .after, .metBy =>
    -- h1 : y.stop < x.start; h2 : z.stop = y.start
    -- z.stop = y.start ≤ y.stop < x.start
    have hze_xa : z.stop < x.start := ltr (h2 ▸ hy) h1
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])
  | .after, .after =>
    -- h1 : y.stop < x.start; h2 : z.stop < y.start
    -- z.stop < y.start ≤ y.stop < x.start
    have hze_xa : z.stop < x.start := ltr h2 (ltr hy h1)
    simp only [compose]; exact mem_cl (cl_after (ltr hz hze_xa) hze_xa) (by simp [List.mem_cons])

end Spindle.Arith
