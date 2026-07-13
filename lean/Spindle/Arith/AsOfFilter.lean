/-
  Spindle As-Of Temporal Filter

  Formalizes `filter_temporal` from `crates/spindle-core/src/pipeline/temporal.rs`
  (the `TemporalFilter` pipeline stage implementing "as-of" reasoning,
  `PrepareOptions.reference_time`):

    for rule in theory.rules() {
        head_active = rule.head.all(|lit|
            lit.temporal_expr.is_some() || lit.temporal.is_empty()
            || lit.temporal.active_at(t));
        body_active = rule.body.all(|bl| match bl.as_logic() {
            Some(lit) => lit.temporal_expr.is_some() || lit.temporal.is_empty()
                         || lit.temporal.active_at(t),
            None => true });   // arithmetic constraints have no temporal
        rule_active = rule.temporal.is_empty() || rule.temporal.active_at(t);
        if rule_active && head_active && body_active { keep }
    }
    // superiority pairs kept iff both endpoints kept

  where `active_at(t) = start <= t && t <= end` — exactly
  `Interval.contains`.

  Key theorems:
  - `filterTemporal_subset`: filtering only removes rules
  - `mem_filterTemporal`: the as-of contract — a rule survives iff it is
    active at the reference time (rule-level AND head AND body windows)
  - `filterTemporal_idempotent`: filtering at the same instant twice
    changes nothing
  - `filterTemporal_atemporal`: non-temporal theories pass through
    unchanged (SPEC-020's non-temporal-equivalence requirement, at the
    filter stage)
  - `filterSup_kept`: a superiority pair survives iff both endpoint rules
    survive
-/
import Spindle.Arith.Interval

namespace Spindle.Arith

/-- A temporal annotation as it appears on rules and literals:
    `none` = atemporal (`Temporal::empty()` in Rust);
    `unresolved` = carries unresolved temporal variables
    (`temporal_expr.is_some()`), which the filter must not judge;
    `window iv` = concrete bounds. -/
inductive TemporalAnn where
  | none
  | unresolved
  | window (iv : Interval)

/-- A literal's filter-relevant shape (name and polarity are irrelevant to
    the as-of filter). -/
structure TLit where
  temporal : TemporalAnn

/-- A body element: a logic literal or an arithmetic constraint (which has
    no temporal and always passes the filter). -/
inductive TBodyElem where
  | logic (l : TLit)
  | arith

/-- The filter-relevant shape of a rule. -/
structure TRule where
  label : String
  temporal : TemporalAnn
  head : List TLit
  body : List TBodyElem

/-- Is an annotation active at the reference time? Mirrors the Rust
    disjunction: unresolved-variables and atemporal annotations always
    pass; concrete windows are checked with `active_at`. -/
def TemporalAnn.activeAt (t : TimePoint) : TemporalAnn → Bool
  | .none => true
  | .unresolved => true
  | .window iv => decide (iv.contains t)

/-- Is a body element active? Arithmetic constraints always pass. -/
def TBodyElem.activeAt (t : TimePoint) : TBodyElem → Bool
  | .logic l => l.temporal.activeAt t
  | .arith => true

/-- The rule-survival predicate: rule-level window AND every head literal
    AND every body element active at `t`. -/
def TRule.activeAt (t : TimePoint) (r : TRule) : Bool :=
  r.temporal.activeAt t
    && r.head.all (fun l => l.temporal.activeAt t)
    && r.body.all (fun b => b.activeAt t)

/-- The as-of filter: keep exactly the rules active at the reference
    time. Mirrors `filter_temporal`'s rule loop. -/
def filterTemporal (t : TimePoint) (rules : List TRule) : List TRule :=
  rules.filter (TRule.activeAt t)

/-- Superiority filtering: a pair survives iff both endpoint rules
    survive. Mirrors the `copy superiorities for kept rules` loop. -/
def filterSup (kept : List TRule) (sup : List (String × String)) :
    List (String × String) :=
  sup.filter fun (w, l) =>
    kept.any (fun r => r.label == w) && kept.any (fun r => r.label == l)

/-! ## The as-of contract -/

/-- Filtering only removes rules; it never adds or alters them. -/
theorem filterTemporal_subset (t : TimePoint) (rules : List TRule) :
    ∀ r ∈ filterTemporal t rules, r ∈ rules := by
  intro r hr
  exact (List.mem_filter.mp hr).1

/-- **The as-of contract**: a rule survives the filter iff it was in the
    theory and is active at the reference time. -/
theorem mem_filterTemporal (t : TimePoint) (rules : List TRule) (r : TRule) :
    r ∈ filterTemporal t rules ↔ r ∈ rules ∧ r.activeAt t = true := by
  simp [filterTemporal, List.mem_filter]

/-- Filtering twice at the same instant is the same as filtering once. -/
theorem filterTemporal_idempotent (t : TimePoint) (rules : List TRule) :
    filterTemporal t (filterTemporal t rules) = filterTemporal t rules := by
  simp [filterTemporal, List.filter_filter]

/-- An annotation with no concrete window is active at every instant. -/
theorem TemporalAnn.activeAt_of_no_window (t : TimePoint) (a : TemporalAnn)
    (ha : ∀ iv, a ≠ .window iv) : a.activeAt t = true := by
  cases a with
  | none => rfl
  | unresolved => rfl
  | window iv => exact absurd rfl (ha iv)

/-- A rule with no concrete windows anywhere is active at every instant. -/
theorem activeAt_of_atemporal (t : TimePoint) (r : TRule)
    (hrule : ∀ iv, r.temporal ≠ .window iv)
    (hhead : ∀ l ∈ r.head, ∀ iv, l.temporal ≠ .window iv)
    (hbody : ∀ b ∈ r.body, ∀ l iv, b = .logic l → l.temporal ≠ .window iv) :
    r.activeAt t = true := by
  unfold TRule.activeAt
  simp only [Bool.and_eq_true, List.all_eq_true]
  refine ⟨⟨?_, ?_⟩, ?_⟩
  · exact TemporalAnn.activeAt_of_no_window t r.temporal hrule
  · intro l hl
    exact TemporalAnn.activeAt_of_no_window t l.temporal (hhead l hl)
  · intro b hb
    cases b with
    | logic l =>
      show (TBodyElem.logic l).activeAt t = true
      simp only [TBodyElem.activeAt]
      exact TemporalAnn.activeAt_of_no_window t l.temporal
        (fun iv => hbody _ hb l iv rfl)
    | arith => rfl

/-- **Non-temporal equivalence at the filter stage**: a theory with no
    concrete temporal windows passes through the as-of filter unchanged. -/
theorem filterTemporal_atemporal (t : TimePoint) (rules : List TRule)
    (h : ∀ r ∈ rules,
      (∀ iv, r.temporal ≠ .window iv)
      ∧ (∀ l ∈ r.head, ∀ iv, l.temporal ≠ .window iv)
      ∧ (∀ b ∈ r.body, ∀ l iv, b = .logic l → l.temporal ≠ .window iv)) :
    filterTemporal t rules = rules := by
  unfold filterTemporal
  apply List.filter_eq_self.mpr
  intro r hr
  obtain ⟨h1, h2, h3⟩ := h r hr
  exact activeAt_of_atemporal t r h1 h2 h3

/-- A superiority pair survives iff both endpoints survive. -/
theorem filterSup_kept (kept : List TRule) (sup : List (String × String))
    (w l : String) :
    (w, l) ∈ filterSup kept sup ↔
      (w, l) ∈ sup
      ∧ (kept.any (fun r => r.label == w)) = true
      ∧ (kept.any (fun r => r.label == l)) = true := by
  simp [filterSup, List.mem_filter]

end Spindle.Arith
