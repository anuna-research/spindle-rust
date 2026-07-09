/-
  Spindle Trust: Temporal Decay and Effective Trust

  Formalizes `DecayModel::apply` and `TrustPolicy::get_effective_trust`
  from `crates/spindle-core/src/trust.rs`.

    pub fn apply(&self, age_secs: f64) -> TrustValue {
        if age_secs <= 0.0 { return 1.0; }
        match self {
            Exponential { half_life_secs } =>
                if half_life <= 0 { 0.0 } else { 0.5.powf(age / half_life) }
            Linear { rate_per_sec } => (1.0 - rate * age).clamp(0.0, 1.0),
            StepFunction { cutoff_secs } => if age < cutoff { 1.0 } else { 0.0 },
        }
    }

  The linear and step models are formalized exactly over ℚ. The
  exponential model requires real powers (0.5^(age/half_life)); since the
  formalization is rational-exact, exponential decay is treated through
  the abstract `DecayLaw` interface: any decay law satisfies the same
  range and effective-trust theorems, and the exponential instance is
  exercised by the Rust unit tests and the trust difftest at Float
  precision. The linear and step models are proven `DecayLaw`s.

  Effective trust (`get_effective_trust`) is base * multiplier; we prove
  it never exceeds the base trust and stays in the unit interval.
-/
import Mathlib.Algebra.Order.Field.Rat
import Mathlib.Tactic.Linarith
import Spindle.Trust.Diminish

namespace Spindle.Trust

/-- Linear decay: `(1 - rate * age).clamp(0.0, 1.0)`, with full trust at
    non-positive age. Mirrors `DecayModel::Linear`. -/
def linearDecay (rate age : ℚ) : TrustValue :=
  if age ≤ 0 then 1 else min (max (1 - rate * age) 0) 1

/-- Step decay: full trust before the cutoff, none after. Mirrors
    `DecayModel::StepFunction`. -/
def stepDecay (cutoff age : ℚ) : TrustValue :=
  if age ≤ 0 then 1 else if age < cutoff then 1 else 0

/-- Effective trust: base trust times the decay multiplier. Mirrors
    `TrustPolicy::get_effective_trust`. -/
def effectiveTrust (base mult : TrustValue) : TrustValue := base * mult

/-! ## Range: decay multipliers always lie in [0, 1] -/

theorem linearDecay_nonneg (rate age : ℚ) : 0 ≤ linearDecay rate age := by
  unfold linearDecay
  split
  · exact zero_le_one
  · exact le_min (le_max_right _ _) zero_le_one

theorem linearDecay_le_one (rate age : ℚ) : linearDecay rate age ≤ 1 := by
  unfold linearDecay
  split
  · exact le_refl 1
  · exact min_le_right _ _

theorem stepDecay_nonneg (cutoff age : ℚ) : 0 ≤ stepDecay cutoff age := by
  unfold stepDecay
  split
  · exact zero_le_one
  · split
    · exact zero_le_one
    · exact le_refl 0

theorem stepDecay_le_one (cutoff age : ℚ) : stepDecay cutoff age ≤ 1 := by
  unfold stepDecay
  split
  · exact le_refl 1
  · split
    · exact le_refl 1
    · exact zero_le_one

/-! ## Freshness: zero (or negative) age means full trust -/

theorem linearDecay_at_zero (rate : ℚ) : linearDecay rate 0 = 1 := by
  simp [linearDecay]

theorem stepDecay_at_zero (cutoff : ℚ) : stepDecay cutoff 0 = 1 := by
  simp [stepDecay]

/-! ## Monotonicity: trust never recovers as testimony ages -/

/-- Linear decay is antitone in age (for a nonnegative decay rate). -/
theorem linearDecay_antitone (rate a₁ a₂ : ℚ) (hrate : 0 ≤ rate) (h : a₁ ≤ a₂) :
    linearDecay rate a₂ ≤ linearDecay rate a₁ := by
  unfold linearDecay
  split
  · -- a₂ ≤ 0, hence a₁ ≤ 0 too
    rename_i h2
    rw [if_pos (le_trans h h2)]
  · rename_i h2
    split
    · -- a₁ ≤ 0: RHS is 1, LHS is clamped ≤ 1
      exact min_le_right _ 1
    · -- both positive: clamp is monotone in the linear expression
      have hmul : rate * a₁ ≤ rate * a₂ := mul_le_mul_of_nonneg_left h hrate
      have : 1 - rate * a₂ ≤ 1 - rate * a₁ := by linarith
      exact min_le_min (max_le_max this (le_refl 0)) (le_refl 1)

/-- Step decay is antitone in age. -/
theorem stepDecay_antitone (cutoff a₁ a₂ : ℚ) (h : a₁ ≤ a₂) :
    stepDecay cutoff a₂ ≤ stepDecay cutoff a₁ := by
  unfold stepDecay
  by_cases h2 : a₂ ≤ 0
  · rw [if_pos h2, if_pos (le_trans h h2)]
  · rw [if_neg h2]
    by_cases hc2 : a₂ < cutoff
    · rw [if_pos hc2]
      by_cases h1 : a₁ ≤ 0
      · rw [if_pos h1]
      · rw [if_neg h1, if_pos (lt_of_le_of_lt h hc2)]
    · rw [if_neg hc2]
      by_cases h1 : a₁ ≤ 0
      · rw [if_pos h1]; exact zero_le_one
      · rw [if_neg h1]
        by_cases hc1 : a₁ < cutoff
        · rw [if_pos hc1]; exact zero_le_one
        · rw [if_neg hc1]

/-! ## Effective trust -/

/-- Decay never increases trust: effective trust is bounded by base trust. -/
theorem effectiveTrust_le_base (base mult : TrustValue)
    (hb : 0 ≤ base) (hm : mult ≤ 1) (_hm0 : 0 ≤ mult) :
    effectiveTrust base mult ≤ base := by
  unfold effectiveTrust
  calc base * mult ≤ base * 1 := mul_le_mul_of_nonneg_left hm hb
  _ = base := mul_one base

/-- Effective trust stays in the unit interval. -/
theorem effectiveTrust_mem_unit (base mult : TrustValue)
    (hb : 0 ≤ base) (hb1 : base ≤ 1) (hm : 0 ≤ mult) (hm1 : mult ≤ 1) :
    0 ≤ effectiveTrust base mult ∧ effectiveTrust base mult ≤ 1 :=
  ⟨mul_nonneg hb hm,
   le_trans (effectiveTrust_le_base base mult hb hm1 hm) hb1⟩

/-! ## The abstract decay-law interface

Any decay model with these four properties supports the effective-trust
theorems above. Linear and step decay are proven instances; the
exponential model 0.5^(age/half-life) satisfies the same interface over ℝ
(range, freshness, antitonicity are standard `rpow` facts) and is
exercised at Float precision by the Rust unit tests. -/

/-- A decay law: a multiplier function of age satisfying the trust-decay
    contract. -/
structure DecayLaw where
  /-- Multiplier as a function of age (seconds). -/
  mult : ℚ → TrustValue
  /-- Multiplier is nonnegative. -/
  nonneg : ∀ a, 0 ≤ mult a
  /-- Multiplier never exceeds one. -/
  le_one : ∀ a, mult a ≤ 1
  /-- Fresh testimony has full trust. -/
  at_zero : mult 0 = 1
  /-- Trust never recovers with age. -/
  antitone : ∀ a₁ a₂, a₁ ≤ a₂ → mult a₂ ≤ mult a₁

/-- Linear decay is a decay law (for nonnegative rates). -/
def DecayLaw.linear (rate : ℚ) (hrate : 0 ≤ rate) : DecayLaw where
  mult := linearDecay rate
  nonneg := linearDecay_nonneg rate
  le_one := linearDecay_le_one rate
  at_zero := linearDecay_at_zero rate
  antitone := fun a₁ a₂ h => linearDecay_antitone rate a₁ a₂ hrate h

/-- Step decay is a decay law. -/
def DecayLaw.step (cutoff : ℚ) : DecayLaw where
  mult := stepDecay cutoff
  nonneg := stepDecay_nonneg cutoff
  le_one := stepDecay_le_one cutoff
  at_zero := stepDecay_at_zero cutoff
  antitone := fun a₁ a₂ h => stepDecay_antitone cutoff a₁ a₂ h

/-- For any decay law, effective trust at any age is bounded by base
    trust and stays in the unit interval. -/
theorem DecayLaw.effective_mem_unit (law : DecayLaw) (base : TrustValue)
    (hb : 0 ≤ base) (_hb1 : base ≤ 1) (age : ℚ) :
    0 ≤ effectiveTrust base (law.mult age)
    ∧ effectiveTrust base (law.mult age) ≤ base := by
  refine ⟨mul_nonneg hb (law.nonneg age), ?_⟩
  exact effectiveTrust_le_base base _ hb (law.le_one age) (law.nonneg age)

end Spindle.Trust
