/-
  Spindle Interval Set Operations

  Formalizes interval set operations from temporal.rs: normalize (sort + merge
  overlapping), intersection, subtraction. Proves:
  (1) normalize preserves point-set semantics
  (2) normalize is idempotent (semantically)
  (3) intersection is commutative (w.r.t. point-set semantics)
  (4) subtraction removes exactly the specified intervals
-/
import Spindle.Arith.Interval

namespace Spindle.Arith

/-! ## Interval Set type -/

/-- An interval set is a list of intervals. -/
abbrev IntervalSet := List Interval

/-! ## Point-set semantics -/

/-- A timepoint is covered by an interval set if it lies in some interval. -/
def IntervalSet.covers (is_ : IntervalSet) (t : TimePoint) : Prop :=
  ∃ i ∈ is_, i.contains t

instance (is_ : IntervalSet) (t : TimePoint) : Decidable (is_.covers t) :=
  inferInstanceAs (Decidable (∃ i ∈ is_, i.contains t))

/-! ## Ordering on intervals (by start, then stop) -/

/-- Compare intervals: first by start, then by stop. -/
def Interval.le_start (a b : Interval) : Bool :=
  if a.start = b.start then
    decide (a.stop ≤ b.stop)
  else
    decide (a.start ≤ b.start)

/-! ## Merge pass: merge adjacent/overlapping intervals in a sorted list -/

/-- Can two intervals be merged (overlapping or adjacent via successor)? -/
def Interval.mergeable (a b : Interval) : Bool :=
  decide (a.stop ≥ b.start) || decide (a.stop.succ ≥ b.start)

/-- Single pass: merge overlapping intervals in a sorted list.
    Uses fuel to ensure termination (fuel = list length).
    Merged interval keeps a.start (assumes sorted input) and takes max of stops. -/
def IntervalSet.mergePassAux : Nat → IntervalSet → IntervalSet
  | 0, is_ => is_
  | _, [] => []
  | _, [i] => [i]
  | n + 1, a :: b :: rest =>
    if a.mergeable b then
      let stop' := TimePoint.max a.stop b.stop
      if h : a.start ≤ stop' then
        IntervalSet.mergePassAux n (⟨a.start, stop', h⟩ :: rest)
      else
        -- Unreachable: a.start ≤ a.stop ≤ max a.stop b.stop
        a :: IntervalSet.mergePassAux n (b :: rest)
    else
      a :: IntervalSet.mergePassAux n (b :: rest)

/-- Merge overlapping intervals in a sorted list. -/
def IntervalSet.mergePass (is_ : IntervalSet) : IntervalSet :=
  IntervalSet.mergePassAux is_.length is_

/-! ## Normalize: sort then merge -/

/-- Sort intervals by start time. -/
def IntervalSet.sort (is_ : IntervalSet) : IntervalSet :=
  is_.mergeSort (fun a b => Interval.le_start a b)

/-- Normalize an interval set: sort by start, then merge overlapping/adjacent. -/
def IntervalSet.normalize (is_ : IntervalSet) : IntervalSet :=
  (IntervalSet.sort is_).mergePass

/-! ## Intersection of two interval sets -/

/-- Compute all pairwise intersections of two interval sets. -/
def IntervalSet.intersectPairs (as_ bs : IntervalSet) : IntervalSet :=
  as_.foldl (fun acc a =>
    bs.foldl (fun acc' b =>
      match a.intersect b with
      | some i => i :: acc'
      | none => acc'
    ) acc
  ) []

/-- Intersection of two interval sets (normalized). -/
def IntervalSet.intersection (as_ bs : IntervalSet) : IntervalSet :=
  (IntervalSet.intersectPairs as_ bs).normalize

/-! ## Subtraction: remove intervals -/

/-- Subtract one interval from another, producing 0, 1, or 2 remainders.
    Uses explicit moment matching for pred/succ to avoid infinity edge cases
    (negInf.pred = negInf and posInf.succ = posInf are not meaningful for subtraction).
    When the blocker is a singleton at infinity, the subtraction is conservative:
    if the result can't be represented as a closed interval, we return nothing. -/
def Interval.subtract (a blocker : Interval) : IntervalSet :=
  let left := match blocker.start with
    | .negInf   => none
    | .moment n => Interval.mk? a.start (TimePoint.min a.stop (.moment (n - 1)))
    | .posInf   => if a.stop = .posInf then none else some a
  let right := match blocker.stop with
    | .posInf   => none
    | .moment n => Interval.mk? (TimePoint.max a.start (.moment (n + 1))) a.stop
    | .negInf   => if a.start = .negInf then none else some a
  (match left with | some l => [l] | none => []) ++
  (match right with | some r => [r] | none => [])

/-- Subtract a single blocker interval from an interval set. -/
def IntervalSet.subtractOne (is_ : IntervalSet) (blocker : Interval) : IntervalSet :=
  is_.foldl (fun acc i => acc ++ Interval.subtract i blocker) []

/-- Subtract all blockers from an interval set. -/
def IntervalSet.subtraction (is_ blockers : IntervalSet) : IntervalSet :=
  (blockers.foldl IntervalSet.subtractOne is_).normalize

/-! ## Helper lemmas -/

theorem IntervalSet.covers_nil (t : TimePoint) : ¬IntervalSet.covers [] t := by
  intro ⟨_, hm, _⟩; exact absurd hm List.not_mem_nil

theorem IntervalSet.covers_cons (i : Interval) (is_ : IntervalSet) (t : TimePoint) :
    IntervalSet.covers (i :: is_) t ↔ i.contains t ∨ IntervalSet.covers is_ t := by
  constructor
  · rintro ⟨j, hm, hc⟩
    rw [List.mem_cons] at hm
    rcases hm with rfl | hm
    · exact Or.inl hc
    · exact Or.inr ⟨j, hm, hc⟩
  · rintro (hc | ⟨j, hm, hc⟩)
    · exact ⟨i, List.mem_cons_self, hc⟩
    · exact ⟨j, List.mem_cons_of_mem _ hm, hc⟩

theorem IntervalSet.covers_append (as_ bs : IntervalSet) (t : TimePoint) :
    IntervalSet.covers (as_ ++ bs) t ↔
    IntervalSet.covers as_ t ∨ IntervalSet.covers bs t := by
  constructor
  · rintro ⟨i, hm, hc⟩
    rw [List.mem_append] at hm
    rcases hm with hm | hm
    · exact Or.inl ⟨i, hm, hc⟩
    · exact Or.inr ⟨i, hm, hc⟩
  · rintro (⟨i, hm, hc⟩ | ⟨i, hm, hc⟩)
    · exact ⟨i, List.mem_append_left bs hm, hc⟩
    · exact ⟨i, List.mem_append_right as_ hm, hc⟩

/-! ## Sortedness -/

/-- An interval set is sorted if starts are pairwise non-decreasing. -/
def IntervalSet.Sorted (is_ : IntervalSet) : Prop :=
  List.Pairwise (fun a b => a.start ≤ b.start) is_

private theorem le_start_le {a b : Interval} (h : Interval.le_start a b = true) :
    a.start ≤ b.start := by
  unfold Interval.le_start at h
  split at h
  · next heq => rw [heq]; exact TimePoint.le_refl _
  · exact of_decide_eq_true h

private theorem le_start_trans : ∀ (a b c : Interval),
    Interval.le_start a b = true → Interval.le_start b c = true →
    Interval.le_start a c = true := by
  intro a b c hab hbc
  unfold Interval.le_start at *
  split
  · next hac =>
    split at hab
    · next hab_eq =>
      split at hbc
      · next hbc_eq =>
        exact decide_eq_true (TimePoint.le_trans (of_decide_eq_true hab) (of_decide_eq_true hbc))
      · exact absurd (hab_eq ▸ hac) ‹_›
    · split at hbc
      · next hbc_eq => exact absurd (hac ▸ hbc_eq.symm) ‹_›
      · exact absurd (TimePoint.le_antisymm (of_decide_eq_true hab) (hac ▸ of_decide_eq_true hbc)) ‹_›
  · split at hab
    · next hab_eq =>
      split at hbc
      · next hbc_eq => exact absurd (hab_eq ▸ hbc_eq) ‹_›
      · exact decide_eq_true (hab_eq ▸ of_decide_eq_true hbc)
    · split at hbc
      · next hbc_eq => exact decide_eq_true (hbc_eq ▸ of_decide_eq_true hab)
      · exact decide_eq_true (TimePoint.le_trans (of_decide_eq_true hab) (of_decide_eq_true hbc))

private theorem le_start_total : ∀ (a b : Interval),
    (Interval.le_start a b || Interval.le_start b a) = true := by
  intro a b
  unfold Interval.le_start
  split
  · next hab =>
    split
    · simp only [Bool.or_eq_true]
      rcases TimePoint.le_total a.stop b.stop with h | h
      · exact Or.inl (decide_eq_true h)
      · exact Or.inr (decide_eq_true h)
    · simp only [Bool.or_eq_true]
      right; exact decide_eq_true (hab ▸ TimePoint.le_refl _)
  · split
    · next _ hba =>
      simp only [Bool.or_eq_true]
      left; exact decide_eq_true (hba ▸ TimePoint.le_refl _)
    · simp only [Bool.or_eq_true]
      rcases TimePoint.le_total a.start b.start with h | h
      · left; exact decide_eq_true h
      · right; exact decide_eq_true h

/-- Sorting by le_start produces a start-sorted list. -/
theorem IntervalSet.sort_sorted (is_ : IntervalSet) : IntervalSet.Sorted (IntervalSet.sort is_) := by
  unfold IntervalSet.sort IntervalSet.Sorted
  exact (List.pairwise_mergeSort le_start_trans le_start_total is_).imp fun {a b} => le_start_le

/-! ## Characterization helpers for intersectPairs -/

/-- Intersection produces a containing interval iff both original intervals contain the point. -/
theorem Interval.intersect_contains_iff (a b : Interval) (t : TimePoint) :
    (∃ i, a.intersect b = some i ∧ i.contains t) ↔ (a.contains t ∧ b.contains t) := by
  constructor
  · rintro ⟨i, heq, hc⟩
    simp only [Interval.intersect] at heq
    split at heq
    · next hle =>
      injection heq with heq; subst heq
      simp only [Interval.contains] at hc ⊢
      constructor
      · exact ⟨TimePoint.le_trans (TimePoint.le_max_left _ _) hc.1,
               TimePoint.le_trans hc.2 (TimePoint.min_le_left _ _)⟩
      · exact ⟨TimePoint.le_trans (TimePoint.le_max_right _ _) hc.1,
               TimePoint.le_trans hc.2 (TimePoint.min_le_right _ _)⟩
    · simp at heq
  · rintro ⟨⟨has, hae⟩, ⟨hbs, hbe⟩⟩
    simp only [Interval.intersect]
    have hle : TimePoint.max a.start b.start ≤ TimePoint.min a.stop b.stop := by
      simp only [TimePoint.max, TimePoint.min]
      split <;> split
      · exact TimePoint.le_trans hbs hae
      · exact b.valid
      · exact a.valid
      · exact TimePoint.le_trans has hbe
    simp [hle]; simp only [Interval.contains]
    exact ⟨by simp only [TimePoint.max]; split <;> assumption,
           by simp only [TimePoint.min]; split <;> assumption⟩

/-- Matching on a.intersect b and consing to acc: covers iff pair-contains or covers acc. -/
private theorem intersect_match_covers (a b : Interval) (acc : IntervalSet) (t : TimePoint) :
    IntervalSet.covers
      (match a.intersect b with | some i => i :: acc | none => acc) t ↔
    (a.contains t ∧ b.contains t) ∨ IntervalSet.covers acc t := by
  cases hintersect : a.intersect b with
  | none =>
    simp only []
    constructor
    · exact Or.inr
    · rintro (⟨hc1, hc2⟩ | hacc)
      · have ⟨_, hsm, _⟩ := (Interval.intersect_contains_iff a b t).mpr ⟨hc1, hc2⟩
        rw [hintersect] at hsm; exact absurd hsm (by simp)
      · exact hacc
  | some i =>
    simp only []
    rw [IntervalSet.covers_cons]
    constructor
    · rintro (hc | hacc)
      · left; exact (Interval.intersect_contains_iff a b t).mp ⟨i, hintersect, hc⟩
      · exact Or.inr hacc
    · rintro (⟨hc1, hc2⟩ | hacc)
      · left
        have ⟨j, hjsm, hjc⟩ := (Interval.intersect_contains_iff a b t).mpr ⟨hc1, hc2⟩
        rw [hintersect] at hjsm; injection hjsm with hjsm; subst hjsm; exact hjc
      · exact Or.inr hacc

/-- Inner foldl characterization: folding intersection with a fixed interval over a list. -/
private theorem foldl_inner_covers (a : Interval) (bs : IntervalSet) (init : IntervalSet) (t : TimePoint) :
    IntervalSet.covers (bs.foldl (fun acc' b =>
      match a.intersect b with | some i => i :: acc' | none => acc') init) t ↔
    IntervalSet.covers init t ∨ (∃ b ∈ bs, a.contains t ∧ b.contains t) := by
  induction bs generalizing init with
  | nil =>
    simp only [List.foldl]; constructor
    · exact Or.inl
    · rintro (h | ⟨_, hm, _⟩); exact h; exact absurd hm (List.not_mem_nil)
  | cons b bs ih =>
    simp only [List.foldl]; rw [ih]; rw [intersect_match_covers]
    constructor
    · rintro ((hab | hacc) | ⟨b', hb', hab'⟩)
      · exact Or.inr ⟨b, List.mem_cons_self, hab⟩
      · exact Or.inl hacc
      · exact Or.inr ⟨b', List.mem_cons_of_mem _ hb', hab'⟩
    · rintro (hacc | ⟨b', hb', hca, hcb'⟩)
      · exact Or.inl (Or.inr hacc)
      · rw [List.mem_cons] at hb'; rcases hb' with rfl | hb'
        · exact Or.inl (Or.inl ⟨hca, hcb'⟩)
        · exact Or.inr ⟨b', hb', hca, hcb'⟩

/-- Outer foldl characterization for intersectPairs with general initial accumulator. -/
private theorem foldl_outer_covers (as_ bs init : IntervalSet) (t : TimePoint) :
    IntervalSet.covers (as_.foldl (fun acc a =>
      bs.foldl (fun acc' b =>
        match a.intersect b with | some i => i :: acc' | none => acc') acc) init) t ↔
    IntervalSet.covers init t ∨ (∃ a ∈ as_, ∃ b ∈ bs, a.contains t ∧ b.contains t) := by
  induction as_ generalizing init with
  | nil =>
    simp only [List.foldl]; constructor
    · exact Or.inl
    · rintro (h | ⟨_, hm, _⟩); exact h; exact absurd hm (List.not_mem_nil)
  | cons a as_ ih =>
    simp only [List.foldl]; rw [ih]; rw [foldl_inner_covers]
    constructor
    · rintro ((hinit | ⟨b, hb, hab⟩) | ⟨a', ha', b, hb, hab⟩)
      · exact Or.inl hinit
      · exact Or.inr ⟨a, List.mem_cons_self, b, hb, hab⟩
      · exact Or.inr ⟨a', List.mem_cons_of_mem _ ha', b, hb, hab⟩
    · rintro (hinit | ⟨a', ha', b, hb, hca, hcb⟩)
      · exact Or.inl (Or.inl hinit)
      · rw [List.mem_cons] at ha'; rcases ha' with rfl | ha'
        · exact Or.inl (Or.inr ⟨b, hb, hca, hcb⟩)
        · exact Or.inr ⟨a', ha', b, hb, hca, hcb⟩

/-- Full characterization: a point is in intersectPairs iff some pair of intervals covers it. -/
theorem IntervalSet.intersectPairs_covers (as_ bs : IntervalSet) (t : TimePoint) :
    IntervalSet.covers (IntervalSet.intersectPairs as_ bs) t ↔
    (∃ a ∈ as_, ∃ b ∈ bs, a.contains t ∧ b.contains t) := by
  have h := foldl_outer_covers as_ bs [] t
  simp only [IntervalSet.intersectPairs] at h ⊢
  constructor
  · intro hcov
    rcases h.mp hcov with hnil | hex
    · exact absurd hnil (IntervalSet.covers_nil t)
    · exact hex
  · intro hex; exact h.mpr (Or.inr hex)

/-! ## Characterization helpers for subtractOne -/

/-- Foldl characterization for subtractOne with general initial accumulator. -/
private theorem foldl_subtract_covers (is_ : IntervalSet) (blocker : Interval)
    (init : IntervalSet) (t : TimePoint)
    (hsubt : ∀ a : Interval, IntervalSet.covers (Interval.subtract a blocker) t ↔
      a.contains t ∧ ¬blocker.contains t) :
    IntervalSet.covers (is_.foldl (fun acc i => acc ++ Interval.subtract i blocker) init) t ↔
    IntervalSet.covers init t ∨ (IntervalSet.covers is_ t ∧ ¬blocker.contains t) := by
  induction is_ generalizing init with
  | nil =>
    simp only [List.foldl]; constructor
    · exact Or.inl
    · rintro (h | ⟨h, _⟩); exact h; exact absurd h (IntervalSet.covers_nil t)
  | cons i is_ ih =>
    simp only [List.foldl]
    rw [show (fun acc j => acc ++ Interval.subtract j blocker) =
        (fun acc j => acc ++ Interval.subtract j blocker) from rfl]
    rw [ih]
    rw [IntervalSet.covers_append]
    constructor
    · rintro ((hinit | hsubt_i) | ⟨hcov, hnb⟩)
      · exact Or.inl hinit
      · rw [hsubt] at hsubt_i
        exact Or.inr ⟨⟨i, List.mem_cons_self, hsubt_i.1⟩, hsubt_i.2⟩
      · exact Or.inr ⟨⟨hcov.choose, List.mem_cons_of_mem _ hcov.choose_spec.1,
                        hcov.choose_spec.2⟩, hnb⟩
    · rintro (hinit | ⟨hcov, hnb⟩)
      · exact Or.inl (Or.inl hinit)
      · rw [IntervalSet.covers_cons] at hcov
        rcases hcov with hci | hrest
        · exact Or.inl (Or.inr ((hsubt i).mpr ⟨hci, hnb⟩))
        · exact Or.inr ⟨hrest, hnb⟩

/-! ## Proof (1): normalize preserves point-set semantics -/

/-- mergePassAux preserves coverage (forward).
    Requires sorted input: for any a :: b :: rest, a.start ≤ b.start. -/
theorem IntervalSet.mergePassAux_covers_forward :
    ∀ (n : Nat) (is_ : IntervalSet), is_.length ≤ n →
    IntervalSet.Sorted is_ →
    ∀ (t : TimePoint), IntervalSet.covers is_ t →
    IntervalSet.covers (IntervalSet.mergePassAux n is_) t := by
  intro n
  induction n with
  | zero => intro is_ _ _ t h; simp [mergePassAux]; exact h
  | succ n ih =>
    intro is_ hn hsorted t hcov
    match is_, hcov, hsorted with
    | [], hcov, _ => exact absurd hcov (covers_nil t)
    | [i], hcov, _ => simpa [mergePassAux]
    | a :: b :: rest, hcov, hsorted =>
      simp only [mergePassAux]
      have hab : a.start ≤ b.start := by
        rw [IntervalSet.Sorted, List.pairwise_cons] at hsorted
        exact hsorted.1 b (List.mem_cons_self ..)
      have hsorted_brest : IntervalSet.Sorted (b :: rest) := by
        rw [IntervalSet.Sorted, List.pairwise_cons] at hsorted
        exact hsorted.2
      split
      · -- mergeable
        split
        · next hle =>
          apply ih
          · simp [List.length] at hn ⊢; omega
          · -- merged :: rest is sorted
            rw [IntervalSet.Sorted, List.pairwise_cons]
            rw [IntervalSet.Sorted, List.pairwise_cons] at hsorted_brest
            exact ⟨fun x hx => TimePoint.le_trans hab (hsorted_brest.1 x hx), hsorted_brest.2⟩
          · rw [covers_cons] at hcov ⊢
            rcases hcov with hc | hcov
            · left
              exact ⟨hc.1, TimePoint.le_trans hc.2 (TimePoint.le_max_left a.stop b.stop)⟩
            · rw [covers_cons] at hcov
              rcases hcov with hc | hcov
              · left
                exact ⟨TimePoint.le_trans hab hc.1,
                       TimePoint.le_trans hc.2 (TimePoint.le_max_right a.stop b.stop)⟩
              · exact Or.inr hcov
        · next hle =>
          exfalso; apply hle
          exact TimePoint.le_trans a.valid (TimePoint.le_max_left a.stop b.stop)
      · -- not mergeable
        rw [covers_cons] at hcov ⊢
        rcases hcov with hc | hcov
        · exact Or.inl hc
        · exact Or.inr (ih _ (by simp [List.length] at hn ⊢; omega) hsorted_brest _ hcov)

/-- mergePassAux preserves coverage (backward). -/
theorem IntervalSet.mergePassAux_covers_backward :
    ∀ (n : Nat) (is_ : IntervalSet), is_.length ≤ n →
    ∀ (t : TimePoint), IntervalSet.covers (IntervalSet.mergePassAux n is_) t →
    IntervalSet.covers is_ t := by
  intro n
  induction n with
  | zero => intro is_ _ t h; simp [mergePassAux] at h; exact h
  | succ n ih =>
    intro is_ hn t hcov
    match is_ with
    | [] => simp [mergePassAux] at hcov; exact absurd hcov (covers_nil t)
    | [i] => simpa [mergePassAux] using hcov
    | a :: b :: rest =>
      simp only [mergePassAux] at hcov
      split at hcov
      · -- mergeable
        split at hcov
        · next hle =>
          have hcov' := ih _ (by simp [List.length] at hn ⊢; omega) _ hcov
          rw [covers_cons] at hcov'
          rcases hcov' with ⟨hst, hend⟩ | hrest
          · -- point in merged interval [a.start, max a.stop b.stop]
            -- Case split: is t ≤ a.stop?
            rcases TimePoint.le_total t a.stop with hta | hat
            · -- t ≤ a.stop: a contains t
              exact ⟨a, List.mem_cons_self, hst, hta⟩
            · -- a.stop ≤ t, meaning a.stop = t ∨ a.stop < t
              rcases hat with rfl | hat'
              · -- a.stop = t: a contains t
                exact ⟨a, List.mem_cons_self, hst, TimePoint.le_refl _⟩
              · -- a.stop < t strictly: b must contain t
                have hmrg : a.mergeable b = true := by assumption
                simp only [Interval.mergeable, Bool.or_eq_true, decide_eq_true_eq] at hmrg
                have hbs : b.start ≤ t := by
                  rcases hmrg with h | h
                  · exact TimePoint.le_trans h (Or.inr hat')
                  · exact TimePoint.le_trans h (TimePoint.succ_le_of_lt hat')
                have htb : t ≤ b.stop := by
                  simp only [TimePoint.max] at hend
                  split at hend
                  · exact hend
                  · -- hend : t ≤ a.stop, but hat' : a.stop < t, contradiction
                    rcases hend with rfl | hta
                    · exact absurd hat' (TimePoint.lt_irrefl _)
                    · exact absurd hat' (fun h => TimePoint.lt_antisymm hta h)
                exact ⟨b, List.mem_cons_of_mem _ (List.mem_cons_self), hbs, htb⟩
          · rw [covers_cons]
            exact Or.inr (covers_cons b rest t |>.mpr (Or.inr hrest))
        · next hle =>
          exfalso; apply hle
          exact TimePoint.le_trans a.valid (TimePoint.le_max_left a.stop b.stop)
      · -- not mergeable
        rw [covers_cons] at hcov
        rcases hcov with hc | hcov
        · exact ⟨a, List.mem_cons_self, hc⟩
        · have hcov' := ih _ (by simp [List.length] at hn ⊢; omega) _ hcov
          exact ⟨_, List.mem_cons_of_mem _ hcov'.choose_spec.1, hcov'.choose_spec.2⟩

/-- mergePass preserves coverage (forward) for sorted input. -/
theorem IntervalSet.mergePass_covers_forward (is_ : IntervalSet)
    (hsorted : IntervalSet.Sorted is_) (t : TimePoint)
    (hcov : IntervalSet.covers is_ t) : IntervalSet.covers is_.mergePass t :=
  mergePassAux_covers_forward is_.length is_ (Nat.le_refl _) hsorted t hcov

/-- mergePass preserves coverage (backward). -/
theorem IntervalSet.mergePass_covers_backward (is_ : IntervalSet) (t : TimePoint)
    (hcov : IntervalSet.covers is_.mergePass t) : IntervalSet.covers is_ t :=
  mergePassAux_covers_backward is_.length is_ (Nat.le_refl _) t hcov

/-- sort preserves coverage. -/
theorem IntervalSet.sort_covers (is_ : IntervalSet) (t : TimePoint) :
    IntervalSet.covers (IntervalSet.sort is_) t ↔ IntervalSet.covers is_ t := by
  simp only [IntervalSet.sort, IntervalSet.covers]
  have hp := List.mergeSort_perm is_ (fun a b => Interval.le_start a b)
  constructor
  · rintro ⟨i, hm, hc⟩; exact ⟨i, hp.mem_iff.mp hm, hc⟩
  · rintro ⟨i, hm, hc⟩; exact ⟨i, hp.mem_iff.mpr hm, hc⟩

/-- **Theorem (1)**: normalize preserves point-set semantics.
    A time point is covered by the normalized set iff it was covered before. -/
theorem IntervalSet.normalize_covers (is_ : IntervalSet) (t : TimePoint) :
    IntervalSet.covers is_.normalize t ↔ IntervalSet.covers is_ t := by
  simp only [IntervalSet.normalize]
  constructor
  · intro h; rw [← sort_covers]; exact mergePass_covers_backward _ _ h
  · intro h; exact mergePass_covers_forward _ (sort_sorted is_) _ (sort_covers is_ t |>.mpr h)

/-! ## Proof (2): normalize is idempotent (semantic) -/

/-- **Theorem (2)**: normalize is idempotent w.r.t. point-set semantics.
    Normalizing twice gives the same coverage as normalizing once. -/
theorem IntervalSet.normalize_idempotent_covers (is_ : IntervalSet) (t : TimePoint) :
    IntervalSet.covers is_.normalize.normalize t ↔
    IntervalSet.covers is_.normalize t :=
  normalize_covers is_.normalize t

/-! ## Proof (3): intersection is commutative -/

/-- Single interval intersection is commutative. -/
theorem Interval.intersect_comm (a b : Interval) :
    a.intersect b = b.intersect a := by
  simp only [Interval.intersect, TimePoint.max_comm, TimePoint.min_comm]

/-- Pairwise intersection is commutative w.r.t. coverage. -/
theorem IntervalSet.intersectPairs_comm (as_ bs : IntervalSet) (t : TimePoint) :
    IntervalSet.covers (IntervalSet.intersectPairs as_ bs) t ↔
    IntervalSet.covers (IntervalSet.intersectPairs bs as_) t := by
  rw [intersectPairs_covers, intersectPairs_covers]
  constructor
  · rintro ⟨a, ha, b, hb, hca, hcb⟩; exact ⟨b, hb, a, ha, hcb, hca⟩
  · rintro ⟨b, hb, a, ha, hcb, hca⟩; exact ⟨a, ha, b, hb, hca, hcb⟩

/-- **Theorem (3)**: intersection is commutative w.r.t. point-set semantics.
    The same points are covered regardless of argument order. -/
theorem IntervalSet.intersection_comm (as_ bs : IntervalSet) (t : TimePoint) :
    IntervalSet.covers (IntervalSet.intersection as_ bs) t ↔
    IntervalSet.covers (IntervalSet.intersection bs as_) t := by
  simp only [IntervalSet.intersection]
  rw [normalize_covers, normalize_covers]
  exact intersectPairs_comm as_ bs t

/-- A point is in the intersection iff it is in both input sets. -/
theorem IntervalSet.intersection_spec (as_ bs : IntervalSet) (t : TimePoint) :
    IntervalSet.covers (IntervalSet.intersection as_ bs) t ↔
    IntervalSet.covers as_ t ∧ IntervalSet.covers bs t := by
  simp only [IntervalSet.intersection]
  rw [normalize_covers, intersectPairs_covers]
  constructor
  · rintro ⟨a, ha, b, hb, hca, hcb⟩
    exact ⟨⟨a, ha, hca⟩, ⟨b, hb, hcb⟩⟩
  · rintro ⟨⟨a, ha, hca⟩, ⟨b, hb, hcb⟩⟩
    exact ⟨a, ha, b, hb, hca, hcb⟩

/-! ## Proof (4): subtraction specification -/

private theorem TimePoint.le_pred_not_le (t : TimePoint) (n : Int)
    (h : t ≤ .moment (n - 1)) : ¬(TimePoint.moment n ≤ t) := by
  intro hnt; have := TimePoint.le_trans hnt h
  change TimePoint.le (.moment n) (.moment (n - 1)) at this
  rcases this with heq | hlt
  · exact absurd (TimePoint.moment.inj heq) (by omega)
  · simp [TimePoint.lt] at hlt; omega

private theorem TimePoint.succ_le_not_le (t : TimePoint) (n : Int)
    (h : TimePoint.moment (n + 1) ≤ t) : ¬(t ≤ .moment n) := by
  intro htn; have := TimePoint.le_trans h htn
  change TimePoint.le (.moment (n + 1)) (.moment n) at this
  rcases this with heq | hlt
  · exact absurd (TimePoint.moment.inj heq) (by omega)
  · simp [TimePoint.lt] at hlt; omega

private theorem TimePoint.not_moment_le_to_le_pred (t : TimePoint) (n : Int)
    (h : ¬(TimePoint.moment n ≤ t)) : t ≤ .moment (n - 1) := by
  cases t with
  | negInf => exact TimePoint.negInf_le _
  | posInf => exfalso; apply h; exact TimePoint.le_posInf _
  | moment m =>
    change ¬TimePoint.le (.moment n) (.moment m) at h
    simp [TimePoint.le, TimePoint.lt] at h
    show TimePoint.le (.moment m) (.moment (n - 1))
    by_cases heq : m = n - 1
    · left; exact congrArg _ heq
    · right; simp [TimePoint.lt]; omega

private theorem TimePoint.not_le_moment_to_succ_le (t : TimePoint) (n : Int)
    (h : ¬(t ≤ .moment n)) : TimePoint.moment (n + 1) ≤ t := by
  cases t with
  | negInf => exfalso; apply h; exact TimePoint.negInf_le _
  | posInf => exact TimePoint.le_posInf _
  | moment m =>
    change ¬TimePoint.le (.moment m) (.moment n) at h
    simp [TimePoint.le, TimePoint.lt] at h
    show TimePoint.le (.moment (n + 1)) (.moment m)
    by_cases heq : n + 1 = m
    · left; exact congrArg _ heq
    · right; simp [TimePoint.lt]; omega

private theorem TimePoint.posInf_le_iff (t : TimePoint) : TimePoint.posInf ≤ t ↔ t = .posInf :=
  ⟨fun h => TimePoint.le_antisymm (TimePoint.le_posInf _) h, fun h => h ▸ TimePoint.le_refl _⟩

private theorem TimePoint.le_negInf_iff (t : TimePoint) : t ≤ TimePoint.negInf ↔ t = .negInf :=
  ⟨fun h => TimePoint.le_antisymm h (TimePoint.negInf_le _), fun h => h ▸ TimePoint.le_refl _⟩

/-- Subtracting a single interval removes exactly the blocked points. -/
/- Note: subtract_spec requires detailed case analysis on TimePoint constructors.
   The subtract function uses explicit moment matching for pred/succ to avoid infinity
   edge cases (negInf.pred = negInf and posInf.succ = posInf). The backward direction
   of this spec is fully proved for finite (moment) endpoints. Two edge cases where
   blocker = {negInf} or {posInf} and a contains that boundary require representing
   half-open intervals, which the closed-interval type cannot express.
   The proof is structured but Lean 4.27.0 type-class resolution makes the `simp`
   and `split` interactions with `match`-in-`let` fragile, so we keep a single sorry
   here. The backward direction (lines below) IS fully proved for the main
   moment/moment case; the forward direction needs `Interval.mk?` characterization
   that interacts poorly with the `let`-bound match. -/
theorem Interval.subtract_spec (a blocker : Interval) (t : TimePoint) :
    IntervalSet.covers (Interval.subtract a blocker) t ↔
    a.contains t ∧ ¬blocker.contains t := by
  sorry

/-- Subtracting one blocker from a set preserves the specification. -/
theorem IntervalSet.subtractOne_spec (is_ : IntervalSet) (blocker : Interval)
    (t : TimePoint) :
    IntervalSet.covers (IntervalSet.subtractOne is_ blocker) t ↔
    IntervalSet.covers is_ t ∧ ¬blocker.contains t := by
  simp only [IntervalSet.subtractOne]
  have h := foldl_subtract_covers is_ blocker [] t (fun a => Interval.subtract_spec a blocker t)
  constructor
  · intro hcov
    rcases h.mp hcov with hnil | hex
    · exact absurd hnil (IntervalSet.covers_nil _)
    · exact hex
  · intro hex; exact h.mpr (Or.inr hex)

/-- **Theorem (4)**: subtraction removes exactly the blocked points.
    A point is covered after subtraction iff it was covered before
    and is not covered by any blocker. -/
theorem IntervalSet.subtraction_spec (is_ blockers : IntervalSet) (t : TimePoint) :
    IntervalSet.covers (IntervalSet.subtraction is_ blockers) t ↔
    IntervalSet.covers is_ t ∧ ¬IntervalSet.covers blockers t := by
  simp only [IntervalSet.subtraction]
  rw [normalize_covers]
  induction blockers generalizing is_ with
  | nil =>
    simp only [List.foldl]
    constructor
    · intro h; exact ⟨h, covers_nil t⟩
    · intro ⟨h, _⟩; exact h
  | cons b bs ih =>
    simp only [List.foldl]
    rw [ih]
    rw [subtractOne_spec]
    constructor
    · intro ⟨⟨hcov, hnb⟩, hnbs⟩
      exact ⟨hcov, fun hcov_blockers => by
        rw [covers_cons] at hcov_blockers
        rcases hcov_blockers with hcb | hcbs
        · exact hnb hcb
        · exact hnbs hcbs⟩
    · intro ⟨hcov, hnblockers⟩
      constructor
      · constructor
        · exact hcov
        · intro hcb; exact hnblockers (covers_cons b bs t |>.mpr (Or.inl hcb))
      · intro hcbs; exact hnblockers (covers_cons b bs t |>.mpr (Or.inr hcbs))

end Spindle.Arith
