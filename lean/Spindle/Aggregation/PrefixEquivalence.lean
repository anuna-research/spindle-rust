import Spindle.Aggregation.GroundBackend

namespace Spindle.Aggregation

def ClosedRules (low : Literal → Prop) (t : Theory) : Prop :=
  ∀ r ∈ t.rules, low r.head → ∀ l ∈ r.body, low l

def SameRules (low : Literal → Prop) (t u : Theory) : Prop :=
  ∀ r, low r.head → (r ∈ t.rules ↔ r ∈ u.rules)

def StateAgree (low : Literal → Prop) (a b : Operational.State) : Prop :=
  ∀ pair, low pair.2 → (pair ∈ a ↔ pair ∈ b)

def ResultAgree (low : Literal → Prop) (a b : Operational.Result) : Prop :=
  ∀ tag l, low l → a.has tag l = b.has tag l

private theorem state_contains_agree (low : Literal → Prop) (a b : Operational.State)
    (h : StateAgree low a b) (pair : ConclusionType × Literal) (hp : low pair.2) :
    a.contains pair = b.contains pair := by
  apply Bool.eq_iff_iff.mpr
  simpa only [List.contains_iff_mem] using h pair hp

private theorem all_on_congr {α : Type} (xs : List α) (f g : α → Bool)
    (h : ∀ x ∈ xs, f x = g x) : xs.all f = xs.all g := by
  apply Bool.eq_iff_iff.mpr
  simp only [List.all_eq_true]
  constructor <;> intro ha x hx
  · exact (h x hx) ▸ ha x hx
  · exact (h x hx).symm ▸ ha x hx

private theorem any_on_congr {α : Type} (xs : List α) (f g : α → Bool)
    (h : ∀ x ∈ xs, f x = g x) : xs.any f = xs.any g := by
  apply Bool.eq_iff_iff.mpr
  simp only [List.any_eq_true]
  constructor <;> rintro ⟨x, hx, ha⟩
  · exact ⟨x, hx, (h x hx) ▸ ha⟩
  · exact ⟨x, hx, (h x hx).symm ▸ ha⟩

private theorem rules_any_agree (low : Literal → Prop) (t u : Theory)
    (same : SameRules low t u) (l : Literal) (hl : low l)
    (f g : Rule → Bool) (test : ∀ r ∈ t.rules, r.head = l → f r = g r) :
    (t.rulesWithHead l).any f = (u.rulesWithHead l).any g := by
  apply Bool.eq_iff_iff.mpr
  simp only [List.any_eq_true, Theory.rulesWithHead, List.mem_filter, beq_iff_eq]
  constructor
  · rintro ⟨r, ⟨hr, head⟩, hf⟩
    exact ⟨r, ⟨(same r (head ▸ hl)).mp hr, head⟩, (test r hr head) ▸ hf⟩
  · rintro ⟨r, ⟨hr, head⟩, hg⟩
    have mem := (same r (head ▸ hl)).mpr hr
    exact ⟨r, ⟨mem, head⟩, (test r mem head).symm ▸ hg⟩

private theorem rules_all_agree (low : Literal → Prop) (t u : Theory)
    (same : SameRules low t u) (l : Literal) (hl : low l)
    (f g : Rule → Bool) (test : ∀ r ∈ t.rules, r.head = l → f r = g r) :
    (t.rulesWithHead l).all f = (u.rulesWithHead l).all g := by
  apply Bool.eq_iff_iff.mpr
  simp only [List.all_eq_true, Theory.rulesWithHead, List.mem_filter, beq_iff_eq]
  constructor
  · intro h r ⟨hr, head⟩
    have mem := (same r (head ▸ hl)).mpr hr
    exact (test r mem head) ▸ h r ⟨mem, head⟩
  · intro h r ⟨hr, head⟩
    have mem := (same r (head ▸ hl)).mp hr
    exact (test r hr head).symm ▸ h r ⟨mem, head⟩


private theorem rules_empty_agree (low : Literal → Prop) (t u : Theory)
    (same : SameRules low t u) (l : Literal) (hl : low l) :
    (t.rulesWithHead l).isEmpty = (u.rulesWithHead l).isEmpty := by
  apply Bool.eq_iff_iff.mpr
  simp only [List.isEmpty_iff]
  constructor <;> intro h
  · apply List.eq_nil_iff_forall_not_mem.mpr
    intro r hr
    have member := List.mem_filter.mp hr
    have head : r.head = l := by simpa using member.2
    have ht : r ∈ t.rulesWithHead l := List.mem_filter.mpr
      ⟨(same r (head ▸ hl)).mpr member.1, member.2⟩
    simp [h] at ht
  · apply List.eq_nil_iff_forall_not_mem.mpr
    intro r hr
    have member := List.mem_filter.mp hr
    have head : r.head = l := by simpa using member.2
    have hu : r ∈ u.rulesWithHead l := List.mem_filter.mpr
      ⟨(same r (head ▸ hl)).mp member.1, member.2⟩
    simp [h] at hu

private theorem holds_agree (low : Literal → Prop) (t u : Theory)
    (same : SameRules low t u) (a b : Operational.State) (h : StateAgree low a b)
    (tag : ConclusionType) (l : Literal) (hl : low l) :
    Operational.holds t a tag l = Operational.holds u b tag l := by
  unfold Operational.holds
  rw [state_contains_agree low a b h (tag, l) hl, rules_empty_agree low t u same l hl]

private theorem body_agree (low : Literal → Prop) (t u : Theory)
    (same : SameRules low t u) (a b : Operational.State) (h : StateAgree low a b)
    (tag : ConclusionType) (r : Rule) (closed : ∀ l ∈ r.body, low l) :
    Operational.bodySat t a tag r = Operational.bodySat u b tag r := by
  apply all_on_congr
  intro l hl
  exact holds_agree low t u same a b h tag l (closed l hl)

private theorem discard_agree (low : Literal → Prop) (t u : Theory)
    (same : SameRules low t u) (a b : Operational.State) (h : StateAgree low a b)
    (tag : ConclusionType) (r : Rule) (closed : ∀ l ∈ r.body, low l) :
    Operational.discarded t a tag r = Operational.discarded u b tag r := by
  apply any_on_congr
  intro l hl
  exact holds_agree low t u same a b h tag l (closed l hl)

private theorem infer_agree (low : Literal → Prop) (t u : Theory)
    (closed : ClosedRules low t) (same : SameRules low t u)
    (priorities : t.superiority = u.superiority) (complements : ∀ l, low l → low l.complement)
    (a b : Operational.State) (h : StateAgree low a b)
    (tag : ConclusionType) (l : Literal) (hl : low l) :
    Operational.infer t a tag l = Operational.infer u b tag l := by
  cases tag with
  | definitelyProvable =>
    apply rules_any_agree low t u same l hl
    intro r hr head
    rw [body_agree low t u same a b h _ r (closed r hr (head ▸ hl))]
  | definitelyNotProvable =>
    apply rules_all_agree low t u same l hl
    intro r hr head
    rw [discard_agree low t u same a b h _ r (closed r hr (head ▸ hl))]
  | defeasiblyProvable =>
    simp only [Operational.infer]
    rw [holds_agree low t u same a b h _ l hl,
      holds_agree low t u same a b h _ l.complement (complements l hl)]
    apply congrArg₂ (· || ·) rfl
    apply congrArg₂ (· && ·)
    · apply congrArg₂ (· && ·) rfl
      apply rules_any_agree low t u same l hl
      intro r hr head
      rw [body_agree low t u same a b h _ r (closed r hr (head ▸ hl))]
    · apply rules_all_agree low t u same l.complement (complements l hl)
      intro r hr head
      rw [discard_agree low t u same a b h _ r (closed r hr (head ▸ complements l hl))]
      apply congrArg₂ (· || ·) rfl
      apply rules_any_agree low t u same l hl
      intro d hd head
      rw [body_agree low t u same a b h _ d (closed d hd (head ▸ hl))]
      simp only [Theory.isSuperior, priorities]
  | defeasiblyNotProvable =>
    simp only [Operational.infer]
    rw [holds_agree low t u same a b h _ l hl,
      holds_agree low t u same a b h _ l.complement (complements l hl)]
    apply congrArg₂ (· && ·) rfl
    apply congrArg₂ (· || ·)
    · apply congrArg₂ (· || ·) rfl
      apply rules_all_agree low t u same l hl
      intro r hr head
      rw [discard_agree low t u same a b h _ r (closed r hr (head ▸ hl))]
    · apply rules_any_agree low t u same l.complement (complements l hl)
      intro r hr head
      rw [body_agree low t u same a b h _ r (closed r hr (head ▸ complements l hl))]
      apply congrArg₂ (· && ·) rfl
      apply rules_all_agree low t u same l hl
      intro d hd head
      rw [discard_agree low t u same a b h _ d (closed d hd (head ▸ hl))]
      simp only [Theory.isSuperior, priorities]

private theorem univ_agree (low : Literal → Prop) (t u : Theory)
    (same : SameRules low t u) : StateAgree low (Operational.univ t) (Operational.univ u) := by
  rintro ⟨tag, l⟩ hl
  simp only [Operational.univ, List.mem_dedup, List.mem_flatMap, List.mem_map, Prod.mk.injEq]
  constructor <;> rintro ⟨r, hr, k, hk, rfl, head⟩
  · exact ⟨r, (same r (head ▸ hl)).mp hr, k, hk, rfl, head⟩
  · exact ⟨r, (same r (head ▸ hl)).mpr hr, k, hk, rfl, head⟩

private theorem step_agree (low : Literal → Prop) (t u : Theory)
    (closed : ClosedRules low t) (same : SameRules low t u)
    (priorities : t.superiority = u.superiority) (complements : ∀ l, low l → low l.complement)
    (a b : Operational.State) (h : StateAgree low a b) :
    StateAgree low (Operational.step t a) (Operational.step u b) := by
  rintro ⟨tag, l⟩ hl
  simp only [Operational.step, List.mem_append, List.mem_filter]
  rw [h (tag, l) hl, univ_agree low t u same (tag, l) hl,
    state_contains_agree low a b h (tag, l) hl,
    infer_agree low t u closed same priorities complements a b h tag l hl]

/-- The four constructive closures agree on dependency-closed domains. -/
theorem operational_close_agrees (low : Literal → Prop) (t u : Theory)
    (closedT : ClosedRules low t) (same : SameRules low t u)
    (priorities : t.superiority = u.superiority)
    (complements : ∀ l, low l → low l.complement) :
    StateAgree low (Operational.close t) (Operational.close u) := by
  let nt := (Operational.univ t).length + 1
  let nu := (Operational.univ u).length + 1
  have steps := FiniteIteration.run_rel (Operational.step t) (Operational.step u)
    (StateAgree low) (fun a b h => step_agree low t u closedT same priorities complements a b h)
    [] [] (fun _ _ => Iff.rfl) (nt + nu)
  have ht := FiniteIteration.close_eq_run_more (Operational.step t)
    (Operational.step_grows t) [] nt nu (Operational.close_fixedpoint t)
  have hu := FiniteIteration.close_eq_run_more (Operational.step u)
    (Operational.step_grows u) [] nu nt (Operational.close_fixedpoint u)
  rw [Nat.add_comm nu nt] at hu
  rw [ht, hu] at steps
  exact steps

/-- Agreement includes all four tags; an undecided literal has no invented negative tag. -/
theorem reason_agrees_on_closed_domains (low : Literal → Prop) (t u : Theory)
    (closedT : ClosedRules low t) (same : SameRules low t u)
    (priorities : t.superiority = u.superiority)
    (complements : ∀ l, low l → low l.complement) :
    ResultAgree low (Operational.reason t) (Operational.reason u) := by
  intro tag l hl
  exact holds_agree low t u same _ _
    (operational_close_agrees low t u closedT same priorities complements) tag l hl

/-- Scheduled prefix evaluation agrees with full-theory reasoning on every tag. -/
theorem groundPrefix_equivalent (owner : Literal → Nat) (theory : Theory) (stage : Nat)
    (ordered : ∀ r ∈ theory.rules, ∀ l ∈ r.body, owner l ≤ owner r.head)
    (complements : ∀ l, owner l.complement = owner l) :
    ResultAgree (fun l => owner l ≤ stage)
      (Operational.reason (groundPrefix owner theory stage)) (Operational.reason theory) := by
  apply reason_agrees_on_closed_domains
  · intro r member head l body
    exact Nat.le_trans (ordered r (List.mem_filter.mp member).1 l body) head
  · intro r hr
    simp [groundPrefix, hr]
  · rfl
  · intro l hl
    simpa only [complements] using hl

private theorem report_filter_agrees (owner : Literal → Nat) (stage : Nat)
    (literals : List Literal) (a b : Operational.Result)
    (h : ResultAgree (fun l => owner l ≤ stage) a b) :
    (reportConclusions literals a).filter (fun c => decide (owner c.literal = stage)) =
    (reportConclusions literals b).filter (fun c => decide (owner c.literal = stage)) := by
  induction literals with
  | nil => rfl
  | cons l rest ih =>
    simp only [reportConclusions, List.flatMap_cons, List.filter_append] at ih ⊢
    rw [ih]
    congr 1
    by_cases owned : owner l = stage
    · have tags : ∀ tag, a.has tag l = b.has tag l := fun tag => h tag l (by omega)
      simp only [reportLiteral, Operational.Result.report, tags]
    · have empty : ∀ r, (reportLiteral r l).filter
          (fun c => decide (owner c.literal = stage)) = [] := by
        intro r
        apply List.filter_eq_nil_iff.mpr
        intro c hc
        have eq := reportLiteral_owned r l c hc
        simp [eq, owned]
      rw [empty a, empty b]

/-- Exact equality of published tagged batches. -/
theorem reasonedStage_equivalent (owner : Literal → Nat) (theory : Theory) (stage : Nat)
    (ordered : ∀ r ∈ theory.rules, ∀ l ∈ r.body, owner l ≤ owner r.head)
    (complements : ∀ l, owner l.complement = owner l) :
    reasonedStage owner theory stage =
      (Operational.reason theory).conclusions.filter (fun c => decide (owner c.literal = stage)) := by
  unfold reasonedStage
  rw [report_filter_agrees owner stage theory.allLiterals _ (Operational.reason theory)
    (groundPrefix_equivalent owner theory stage ordered complements), reason_reports]

private theorem advanceGround (owner : Literal → Nat) (theory : Theory)
    (recognized : groundScheduled owner theory = true) (state : StageState) :
    advanceStage owner (groundBackend owner theory) state =
      .ok ⟨state.next + 1, state.conclusions ++ reasonedStage owner theory state.next⟩ := by
  have owned : (reasonedStage owner theory state.next).all
      (fun c => decide (owner c.literal = state.next)) = true := by
    apply List.all_eq_true.mpr
    intro c member
    exact decide_eq_true ((mem_reasonedStage owner theory state.next c).mp member).2
  simp only [advanceStage, groundBackend, recognized, if_true, owned]

/-- Accumulating scheduled batches preserves exactly the full-theory tags for
the stages visited, in addition to the caller's prior conclusions. -/
theorem runGround_membership (owner : Literal → Nat) (theory : Theory)
    (recognized : groundScheduled owner theory = true)
    (complements : ∀ l, owner l.complement = owner l)
    (count : Nat) (before after : StageState)
    (returned : runStages owner (groundBackend owner theory) count before = .ok after)
    (c : Conclusion) :
    c ∈ after.conclusions ↔ c ∈ before.conclusions ∨
      (c ∈ (Operational.reason theory).conclusions ∧ before.next ≤ owner c.literal ∧
        owner c.literal < before.next + count) := by
  induction count generalizing before with
  | zero =>
    cases returned
    simp
  | succ count ih =>
    simp only [runStages, advanceGround owner theory recognized] at returned
    rw [ih _ returned]
    rw [reasonedStage_equivalent owner theory before.next
      ((groundScheduled_iff _ _).mp recognized).2.1 complements]
    simp only [List.mem_append, List.mem_filter, decide_eq_true_eq]
    constructor
    · intro member
      rcases member with (old | ⟨full, same⟩) | ⟨full, lower, upper⟩
      · exact Or.inl old
      · exact Or.inr ⟨full, by omega, by omega⟩
      · exact Or.inr ⟨full, by omega, by omega⟩
    · intro member
      rcases member with old | ⟨full, lower, upper⟩
      · exact Or.inl (Or.inl old)
      · by_cases same : owner c.literal = before.next
        · exact Or.inl (Or.inr ⟨full, same⟩)
        · exact Or.inr ⟨full, by omega, by omega⟩

/-- Complete execution reports all four tags exactly as a full-theory run,
restricted only by the explicitly requested number of stages. -/
theorem executeGround_membership (owner : Literal → Nat) (theory : Theory)
    (recognized : groundScheduled owner theory = true)
    (complements : ∀ l, owner l.complement = owner l)
    (count : Nat) (after : StageState)
    (returned : executeStages owner (groundBackend owner theory) count = .ok after)
    (c : Conclusion) : c ∈ after.conclusions ↔
      c ∈ (Operational.reason theory).conclusions ∧ owner c.literal < count := by
  simpa using runGround_membership owner theory recognized complements count ⟨0, []⟩ after returned c

end Spindle.Aggregation
