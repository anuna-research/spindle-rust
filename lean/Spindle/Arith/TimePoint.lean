/-
  Spindle TimePoint Type

  Defines the TimePoint type for temporal reasoning: NegInf, Moment(Int), PosInf.
  Provides a total ordering with NegInf < Moment(n) < PosInf, with proofs of
  totality, transitivity, and antisymmetry. Includes successor/predecessor
  for closed-interval arithmetic.
-/

namespace Spindle.Arith

/-! ## TimePoint type -/

/-- A point on the extended integer timeline. -/
inductive TimePoint where
  | negInf                -- negative infinity
  | moment (n : Int)      -- finite integer moment
  | posInf                -- positive infinity
  deriving Repr, BEq, Inhabited

/-! ## Ordering -/

/-- Strict less-than on TimePoints. -/
def TimePoint.lt : TimePoint → TimePoint → Prop
  | .negInf,    .moment _  => True
  | .negInf,    .posInf    => True
  | .moment _,  .posInf    => True
  | .moment a,  .moment b  => a < b
  | _,          _          => False

/-- Less-than-or-equal on TimePoints. -/
def TimePoint.le (a b : TimePoint) : Prop :=
  a = b ∨ TimePoint.lt a b

instance : LT TimePoint := ⟨TimePoint.lt⟩
instance : LE TimePoint := ⟨TimePoint.le⟩

@[simp] theorem TimePoint.lt_unfold (a b : TimePoint) : (a < b) = TimePoint.lt a b := rfl
@[simp] theorem TimePoint.le_unfold (a b : TimePoint) : (a ≤ b) = TimePoint.le a b := rfl

/-! ## Decidability -/

instance : DecidableEq TimePoint
  | .negInf,   .negInf   => isTrue rfl
  | .negInf,   .moment _ => isFalse (fun h => TimePoint.noConfusion h)
  | .negInf,   .posInf   => isFalse (fun h => TimePoint.noConfusion h)
  | .moment _, .negInf   => isFalse (fun h => TimePoint.noConfusion h)
  | .moment a, .moment b =>
    if h : a = b then isTrue (congrArg TimePoint.moment h)
    else isFalse (fun h' => h (TimePoint.moment.inj h'))
  | .moment _, .posInf   => isFalse (fun h => TimePoint.noConfusion h)
  | .posInf,   .negInf   => isFalse (fun h => TimePoint.noConfusion h)
  | .posInf,   .moment _ => isFalse (fun h => TimePoint.noConfusion h)
  | .posInf,   .posInf   => isTrue rfl

instance (a b : TimePoint) : Decidable (TimePoint.lt a b) := by
  cases a <;> cases b <;> simp [TimePoint.lt] <;> exact inferInstance

instance (a b : TimePoint) : Decidable (a < b) := by
  show Decidable (TimePoint.lt a b); exact inferInstance

instance (a b : TimePoint) : Decidable (a ≤ b) := by
  show Decidable (TimePoint.le a b); simp [TimePoint.le]; exact inferInstance

/-! ## Ordering properties -/

/-- TimePoint ordering is irreflexive. -/
theorem TimePoint.lt_irrefl (a : TimePoint) : ¬(a < a) := by
  cases a <;> simp [TimePoint.lt]

/-- TimePoint ordering is transitive. -/
theorem TimePoint.lt_trans {a b c : TimePoint} (hab : a < b) (hbc : b < c) : a < c := by
  change TimePoint.lt a b at hab
  change TimePoint.lt b c at hbc
  show TimePoint.lt a c
  cases a <;> cases b <;> cases c <;> simp_all [TimePoint.lt]
  · exact Int.lt_trans hab hbc

/-- TimePoint ordering is antisymmetric. -/
theorem TimePoint.lt_antisymm {a b : TimePoint} (hab : a < b) (hba : b < a) : False := by
  change TimePoint.lt a b at hab
  change TimePoint.lt b a at hba
  cases a <;> cases b <;> simp_all [TimePoint.lt]
  · exact absurd (Int.lt_trans hab hba) (Int.lt_irrefl _)

/-- TimePoint ordering is total: for any a b, either a < b, a = b, or b < a. -/
theorem TimePoint.lt_trichotomy (a b : TimePoint) : a < b ∨ a = b ∨ b < a := by
  show TimePoint.lt a b ∨ a = b ∨ TimePoint.lt b a
  cases a <;> cases b <;> simp [TimePoint.lt]
  case moment.moment n m =>
    rcases Int.lt_trichotomy n m with h | h | h
    · exact Or.inl h
    · exact Or.inr (Or.inl h)
    · exact Or.inr (Or.inr h)

/-- TimePoint ≤ is reflexive. -/
theorem TimePoint.le_refl (a : TimePoint) : a ≤ a :=
  show TimePoint.le a a from Or.inl rfl

/-- TimePoint ≤ is transitive. -/
theorem TimePoint.le_trans {a b c : TimePoint} (hab : a ≤ b) (hbc : b ≤ c) : a ≤ c := by
  change TimePoint.le a b at hab
  change TimePoint.le b c at hbc
  show TimePoint.le a c
  simp [TimePoint.le] at *
  rcases hab with rfl | hab
  · exact hbc
  · rcases hbc with rfl | hbc
    · exact Or.inr hab
    · exact Or.inr (TimePoint.lt_trans hab hbc)

/-- TimePoint ≤ is antisymmetric. -/
theorem TimePoint.le_antisymm {a b : TimePoint} (hab : a ≤ b) (hba : b ≤ a) : a = b := by
  change TimePoint.le a b at hab
  change TimePoint.le b a at hba
  simp [TimePoint.le] at *
  rcases hab with rfl | hab
  · rfl
  · rcases hba with rfl | hba
    · rfl
    · exact absurd hab (fun h => TimePoint.lt_antisymm h hba)

/-- TimePoint ≤ is total. -/
theorem TimePoint.le_total (a b : TimePoint) : a ≤ b ∨ b ≤ a := by
  show TimePoint.le a b ∨ TimePoint.le b a
  simp [TimePoint.le]
  rcases TimePoint.lt_trichotomy a b with h | rfl | h
  · exact Or.inl (Or.inr h)
  · exact Or.inl (Or.inl rfl)
  · exact Or.inr (Or.inr h)

/-! ## Successor and predecessor -/

/-- Successor of a timepoint (for closed-interval right-step). -/
def TimePoint.succ : TimePoint → TimePoint
  | .negInf   => .negInf
  | .moment n => .moment (n + 1)
  | .posInf   => .posInf

/-- Predecessor of a timepoint (for closed-interval left-step). -/
def TimePoint.pred : TimePoint → TimePoint
  | .negInf   => .negInf
  | .moment n => .moment (n - 1)
  | .posInf   => .posInf

/-! ## Successor/predecessor properties -/

/-- Successor of a finite moment is strictly greater. -/
theorem TimePoint.lt_succ_moment (n : Int) :
    TimePoint.lt (TimePoint.moment n) (TimePoint.moment (n + 1)) := by
  simp [TimePoint.lt]; omega

/-- Predecessor of a finite moment is strictly less. -/
theorem TimePoint.pred_lt_moment (n : Int) :
    TimePoint.lt (TimePoint.moment (n - 1)) (TimePoint.moment n) := by
  simp [TimePoint.lt]; omega

/-- pred is left-inverse of succ on finite moments. -/
theorem TimePoint.pred_succ (n : Int) :
    (TimePoint.moment n).succ.pred = TimePoint.moment n := by
  simp [TimePoint.succ, TimePoint.pred]

/-- succ is left-inverse of pred on finite moments. -/
theorem TimePoint.succ_pred (n : Int) :
    (TimePoint.moment n).pred.succ = TimePoint.moment n := by
  simp [TimePoint.succ, TimePoint.pred]

/-- succ is monotone: a < b → succ a ≤ succ b. -/
theorem TimePoint.succ_mono {a b : TimePoint} (h : a < b) : a.succ ≤ b.succ := by
  change TimePoint.lt a b at h
  show TimePoint.le a.succ b.succ
  cases a <;> cases b <;> simp_all [TimePoint.lt, TimePoint.succ, TimePoint.le]

/-- NegInf is the bottom element. -/
theorem TimePoint.negInf_le (a : TimePoint) : TimePoint.negInf ≤ a := by
  show TimePoint.le TimePoint.negInf a
  cases a <;> simp [TimePoint.le, TimePoint.lt]

/-- PosInf is the top element. -/
theorem TimePoint.le_posInf (a : TimePoint) : a ≤ TimePoint.posInf := by
  show TimePoint.le a TimePoint.posInf
  cases a <;> simp [TimePoint.le, TimePoint.lt]

/-- If a < b then a.succ ≤ b. -/
theorem TimePoint.succ_le_of_lt {a b : TimePoint} (h : TimePoint.lt a b) : a.succ ≤ b := by
  show TimePoint.le a.succ b
  simp [TimePoint.le]
  cases a <;> cases b <;> simp_all [TimePoint.lt, TimePoint.succ, TimePoint.lt]
  · omega

end Spindle.Arith
