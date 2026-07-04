/-
  Spindle Trust: The Diminishment Operator

  Formalizes graduated defeat from the trust module
  (`crates/spindle-core/src/trust.rs`, `DiminisherInfo`).

  The Rust implementation computes:
    diminishment     = min(defeater_degree * target_degree, target_degree)
    resulting_degree = max(target_degree - diminishment, 0)

  We mirror that computation exactly, then prove it coincides with the
  paper's multiplicative operator τ_c(1 − τ_d) on the unit interval
  (`diminish_eq_mul`), and establish the five "Properties of diminishment"
  from the paper plus Pollock's constraints:

    1. Monotonicity in defeater strength   (diminish_antitone)
    2. Bounded reduction                    (diminish_nonneg, diminish_le_self)
    3. Full defeat as limit case            (diminish_one)
    4. Zero defeat as limit case            (diminish_zero)
    5. Proportionality                      (explicit in diminish_eq_mul)

  Pollock's constraints: J(x,y) ≤ x (diminish_le_self), J(x,0) = x
  (diminish_zero); the deliberately relaxed third constraint is witnessed
  by diminish_pos: a defeater with d < 1 never fully obliterates a
  positive conclusion, no matter how it compares to the target.

  Multiple diminishers compose commutatively and order-independently
  (diminish_diminish_comm, diminishAll_eq_prod): the result is the
  product form c · ∏(1 − dᵢ).
-/
import Mathlib.Algebra.Order.Field.Rat
import Mathlib.Tactic.Linarith
import Mathlib.Tactic.Ring

namespace Spindle.Trust

/-- Trust values. The Rust engine uses f64; the model uses exact rationals.
    The meaningful range is [0, 1]. -/
abbrev TrustValue := ℚ

/-- The reduction amount — mirrors `DiminisherInfo::new`:
    `diminishment = (defeater_degree * target_degree).min(target_degree)`. -/
def diminishment (c d : TrustValue) : TrustValue := min (d * c) c

/-- The resulting degree — mirrors `DiminisherInfo::resulting_degree`
    (non-full-defeat case): `(target_degree - diminishment).max(0.0)`. -/
def diminish (c d : TrustValue) : TrustValue := max (c - diminishment c d) 0

/-- **Faithfulness**: on the unit interval, the implementation's
    clamped-subtraction form equals the paper's multiplicative operator
    τ_c(1 − τ_d). -/
theorem diminish_eq_mul (c d : TrustValue) (hc : 0 ≤ c) (hd : 0 ≤ d) (hd1 : d ≤ 1) :
    diminish c d = c * (1 - d) := by
  unfold diminish diminishment
  have h1 : d * c ≤ c := by
    calc d * c ≤ 1 * c := mul_le_mul_of_nonneg_right hd1 hc
    _ = c := one_mul c
  rw [min_eq_left h1]
  have h2 : 0 ≤ c - d * c := by linarith
  rw [max_eq_left h2]
  ring

/-- Diminishment never produces a negative degree (bounded below). -/
theorem diminish_nonneg (c d : TrustValue) : 0 ≤ diminish c d :=
  le_max_right _ _

/-- **Pollock constraint 1 / bounded reduction**: diminishment never
    increases trust: J(c, d) ≤ c. (The nonnegativity of d is required:
    a "negative-trust" defeater would otherwise increase the degree.) -/
theorem diminish_le_self (c d : TrustValue) (hc : 0 ≤ c) (hd : 0 ≤ d) :
    diminish c d ≤ c := by
  unfold diminish diminishment
  have h0 : 0 ≤ min (d * c) c := le_min (mul_nonneg hd hc) hc
  exact max_le (by linarith) hc

/-- **Pollock constraint 2 / zero defeat as limit case**:
    an incredible defeater (d = 0) leaves the conclusion untouched. -/
theorem diminish_zero (c : TrustValue) (hc : 0 ≤ c) : diminish c 0 = c := by
  rw [diminish_eq_mul c 0 hc le_rfl zero_le_one]; ring

/-- **Full defeat as limit case**: a maximally trusted defeater (d = 1)
    recovers classical binary defeat. -/
theorem diminish_one (c : TrustValue) (hc : 0 ≤ c) : diminish c 1 = 0 := by
  rw [diminish_eq_mul c 1 hc zero_le_one le_rfl]; ring

/-- **Monotonicity in defeater strength**: a more credible defeater causes
    a greater (or equal) reduction. -/
theorem diminish_antitone (c d₁ d₂ : TrustValue) (hc : 0 ≤ c)
    (h1 : 0 ≤ d₁) (h12 : d₁ ≤ d₂) (h2 : d₂ ≤ 1) :
    diminish c d₂ ≤ diminish c d₁ := by
  rw [diminish_eq_mul c d₁ hc h1 (le_trans h12 h2),
      diminish_eq_mul c d₂ hc (le_trans h1 h12) h2]
  have : 1 - d₂ ≤ 1 - d₁ := by linarith
  exact mul_le_mul_of_nonneg_left this hc

/-- **Relaxed third constraint** (deliberate deviation from Pollock): a
    defeater short of full trust (d < 1) never fully obliterates a positive
    conclusion — even a defeater more trusted than the target only
    diminishes it. Full defeat occurs only at d = 1 (`diminish_one`). -/
theorem diminish_pos (c d : TrustValue) (hc : 0 < c) (hd : 0 ≤ d) (hd1 : d < 1) :
    0 < diminish c d := by
  rw [diminish_eq_mul c d (le_of_lt hc) hd (le_of_lt hd1)]
  have : 0 < 1 - d := by linarith
  exact mul_pos hc this

/-- Diminishment preserves the unit interval. -/
theorem diminish_mem_unit (c d : TrustValue) (hc : 0 ≤ c) (hc1 : c ≤ 1) (hd : 0 ≤ d) :
    0 ≤ diminish c d ∧ diminish c d ≤ 1 :=
  ⟨diminish_nonneg c d, le_trans (diminish_le_self c d hc hd) hc1⟩

/-! ## Multiple diminishers -/

/-- Sequential application of two diminishers is order-independent. -/
theorem diminish_diminish_comm (c d₁ d₂ : TrustValue) (hc : 0 ≤ c)
    (h1 : 0 ≤ d₁) (h1' : d₁ ≤ 1) (h2 : 0 ≤ d₂) (h2' : d₂ ≤ 1) :
    diminish (diminish c d₁) d₂ = diminish (diminish c d₂) d₁ := by
  have hc1 : 0 ≤ diminish c d₁ := diminish_nonneg c d₁
  have hc2 : 0 ≤ diminish c d₂ := diminish_nonneg c d₂
  rw [diminish_eq_mul _ d₂ hc1 h2 h2', diminish_eq_mul c d₁ hc h1 h1',
      diminish_eq_mul _ d₁ hc2 h1 h1', diminish_eq_mul c d₂ hc h2 h2']
  ring

/-- Fold a list of diminishers over a conclusion degree. -/
def diminishAll (c : TrustValue) (ds : List TrustValue) : TrustValue :=
  ds.foldl diminish c

/-- **Product form**: applying diminishers d₁, …, dₙ yields
    c · ∏(1 − dᵢ). Order-independence of `diminishAll` is immediate,
    since multiplication commutes. -/
theorem diminishAll_eq_prod (c : TrustValue) (ds : List TrustValue)
    (hc : 0 ≤ c) (hds : ∀ d ∈ ds, 0 ≤ d ∧ d ≤ 1) :
    diminishAll c ds = c * (ds.map (fun d => 1 - d)).prod := by
  induction ds generalizing c with
  | nil => simp [diminishAll]
  | cons d ds ih =>
    have hd := hds d List.mem_cons_self
    have hrest : ∀ x ∈ ds, 0 ≤ x ∧ x ≤ 1 :=
      fun x hx => hds x (List.mem_cons_of_mem d hx)
    have hc' : 0 ≤ diminish c d := diminish_nonneg c d
    simp only [diminishAll, List.foldl_cons, List.map_cons, List.prod_cons]
    rw [show ds.foldl diminish (diminish c d) = diminishAll (diminish c d) ds from rfl,
        ih (diminish c d) hc' hrest, diminish_eq_mul c d hc hd.1 hd.2]
    ring

theorem list_prod_nonneg (l : List ℚ) (h : ∀ a ∈ l, 0 ≤ a) :
    0 ≤ l.prod := by
  induction l with
  | nil => simp
  | cons x xs ih =>
    simp only [List.prod_cons]
    exact mul_nonneg (h x List.mem_cons_self)
      (ih fun a ha => h a (List.mem_cons_of_mem _ ha))

theorem list_prod_le_one (l : List ℚ) (h0 : ∀ a ∈ l, 0 ≤ a)
    (h1 : ∀ a ∈ l, a ≤ 1) : l.prod ≤ 1 := by
  induction l with
  | nil => simp
  | cons x xs ih =>
    simp only [List.prod_cons]
    have hx0 := h0 x List.mem_cons_self
    have hx1 := h1 x List.mem_cons_self
    have hxs0 : ∀ a ∈ xs, 0 ≤ a := fun a ha => h0 a (List.mem_cons_of_mem _ ha)
    have hxs1 : ∀ a ∈ xs, a ≤ 1 := fun a ha => h1 a (List.mem_cons_of_mem _ ha)
    calc x * xs.prod ≤ 1 * 1 := by
          exact mul_le_mul hx1 (ih hxs0 hxs1) (list_prod_nonneg xs hxs0) zero_le_one
    _ = 1 := one_mul 1

/-- Collective diminishment is at least as strong as any individual
    diminisher: the fold result is bounded by each single application. -/
theorem diminishAll_le_single (c : TrustValue) (d : TrustValue) (ds : List TrustValue)
    (hc : 0 ≤ c) (hd : 0 ≤ d ∧ d ≤ 1) (hds : ∀ x ∈ ds, 0 ≤ x ∧ x ≤ 1)
    (hmem : d ∈ ds) :
    diminishAll c ds ≤ diminish c d := by
  rw [diminishAll_eq_prod c ds hc hds, diminish_eq_mul c d hc hd.1 hd.2]
  -- The full product is bounded by the single factor (1 - d), since every
  -- other factor lies in [0, 1].
  have hprod : (ds.map (fun x => 1 - x)).prod ≤ 1 - d := by
    induction ds with
    | nil => cases hmem
    | cons y ys ih =>
      have hy := hds y List.mem_cons_self
      have hys : ∀ x ∈ ys, 0 ≤ x ∧ x ≤ 1 :=
        fun x hx => hds x (List.mem_cons_of_mem y hx)
      have hfac0 : ∀ a ∈ ys.map (fun x => 1 - x), (0:ℚ) ≤ a := by
        intro a ha
        simp only [List.mem_map] at ha
        obtain ⟨x, hx, rfl⟩ := ha
        have := hys x hx
        linarith [this.2]
      have hfac1 : ∀ a ∈ ys.map (fun x => 1 - x), a ≤ (1:ℚ) := by
        intro a ha
        simp only [List.mem_map] at ha
        obtain ⟨x, hx, rfl⟩ := ha
        have := hys x hx
        linarith [this.1]
      have hprod_nonneg : 0 ≤ (ys.map (fun x => 1 - x)).prod :=
        list_prod_nonneg _ hfac0
      have hprod_le_one : (ys.map (fun x => 1 - x)).prod ≤ 1 :=
        list_prod_le_one _ hfac0 hfac1
      simp only [List.map_cons, List.prod_cons]
      rcases List.mem_cons.mp hmem with rfl | hmem'
      · calc (1 - d) * (ys.map (fun x => 1 - x)).prod
            ≤ (1 - d) * 1 := by
              apply mul_le_mul_of_nonneg_left hprod_le_one
              linarith [hy.2]
        _ = 1 - d := mul_one _
      · calc (1 - y) * (ys.map (fun x => 1 - x)).prod
            ≤ 1 * (ys.map (fun x => 1 - x)).prod := by
              apply mul_le_mul_of_nonneg_right _ hprod_nonneg
              linarith [hy.1]
        _ = (ys.map (fun x => 1 - x)).prod := one_mul _
        _ ≤ 1 - d := ih hys hmem'
  exact mul_le_mul_of_nonneg_left hprod hc

end Spindle.Trust
