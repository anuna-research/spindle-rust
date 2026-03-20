import SpindleLean.Reason
import SpindleLean.Properties.Soundness
import SpindleLean.Properties.Consistency

/-!
# SpindleLean.AmbiguityBlocking

Ambiguity blocking semantics for defeasible reasoning.

## Overview

In ambiguity blocking (AB), when two opposing defeasible rules are both
applicable and neither is superior, both literals get `-d` (they are
"blocked"). This is already the default mode in the Lambda closure
computation (`Lambda.lambdaStep`).

## Ambiguity Locality

The key property proved here is **Ambiguity Locality**: if literals `p` and
`~p` are both ambiguous (both `-d`), and a rule `r: => q` has a body that
does not mention `p` or `~p`, then `q`'s provability is independent of the
`p`/`~p` ambiguity.

More precisely: if `q ∈ +d` and no rule for `q` has `p` or `~p` in its body,
then `q`'s membership in `+d` does not depend on whether `p` and `~p` are
ambiguous or not. This is formalized as: the `canProve` check for `q` does
not inspect `p` or `~p` when they do not appear in any relevant rule body.

## Test theory

`r1: => p, r2: => ~p, r3: => q` — `p` and `~p` are ambiguous (both `-d`),
while `q` is `+d` because `r3` is an unconditional defeasible rule with no
attackers.
-/

namespace AmbiguityBlocking

open Delta Lambda Soundness Consistency

/-! ### Private helper lemmas -/

/-- `List.all` is preserved by pointwise-equal functions (membership-restricted). -/
private theorem List.all_mem_congr {α : Type u_1} {l : List α} {f g : α → Bool}
    (h : ∀ x ∈ l, f x = g x) : l.all f = l.all g := by
  induction l with
  | nil => rfl
  | cons hd tl ih =>
    simp only [List.all_cons]
    rw [h hd (List.mem_cons.mpr (Or.inl rfl)),
        ih (fun x hx => h x (List.mem_cons.mpr (Or.inr hx)))]

/-- `List.any` is preserved by pointwise-equal functions (membership-restricted). -/
private theorem List.any_mem_congr {α : Type u_1} {l : List α} {f g : α → Bool}
    (h : ∀ x ∈ l, f x = g x) : l.any f = l.any g := by
  induction l with
  | nil => rfl
  | cons hd tl ih =>
    simp only [List.any_cons]
    rw [h hd (List.mem_cons.mpr (Or.inl rfl)),
        ih (fun x hx => h x (List.mem_cons.mpr (Or.inr hx)))]

/-- `List.contains` reflects membership iff: equal membership implies equal contains. -/
private theorem list_contains_iff_eq {α : Type u_1} [BEq α] [LawfulBEq α]
    {l : α} {s₁ s₂ : List α}
    (h : l ∈ s₁ ↔ l ∈ s₂) :
    s₁.contains l = s₂.contains l := by
  rw [Bool.eq_iff_iff, List.contains_iff_mem, List.contains_iff_mem]
  exact h

/-! ### Test theory: ambiguity blocking demonstration -/

/-- Test theory: `r1: => p, r2: => ~p, r3: => q`.
    `p` and `~p` block each other; `q` is independently provable. -/
def ambiguityTest : Theory :=
  { rules := [
      Rule.mkDefeasible "r1" [] (Literal.pos "p"),
      Rule.mkDefeasible "r2" [] (Literal.neg "p"),
      Rule.mkDefeasible "r3" [] (Literal.pos "q")
    ],
    superiority := [] }

/-- Helper: check that a conclusion appears in the output. -/
def hasConclusion (cs : List Conclusion) (ct : ConclusionType) (l : Literal) : Bool :=
  cs.any fun c => c.conclusionType == ct && c.literal == l

-- Evaluate the full reasoning output
#eval reason ambiguityTest

-- p and ~p are not in +D (no strict rules)
#guard !hasConclusion (reason ambiguityTest) .plusD (Literal.pos "p")
#guard !hasConclusion (reason ambiguityTest) .plusD (Literal.neg "p")

-- p and ~p are in -D (no strict derivation)
#guard hasConclusion (reason ambiguityTest) .minusD (Literal.pos "p")
#guard hasConclusion (reason ambiguityTest) .minusD (Literal.neg "p")

-- p and ~p block each other: both are -d (ambiguity blocking)
#guard hasConclusion (reason ambiguityTest) .minusd (Literal.pos "p")
#guard hasConclusion (reason ambiguityTest) .minusd (Literal.neg "p")

-- q is not +D (only defeasible rule)
#guard !hasConclusion (reason ambiguityTest) .plusD (Literal.pos "q")

-- q IS +d: independently provable despite p/~p ambiguity
#guard hasConclusion (reason ambiguityTest) .plusd (Literal.pos "q")

/-! ### Ambiguity Locality theorem -/

/-- A literal `p` is **irrelevant** to a rule `r` if neither `p` nor `~p`
    appears in `r`'s body. -/
def irrelevant (p : Literal) (r : Rule) : Prop :=
  p ∉ r.body ∧ p.complement ∉ r.body

/-- A literal `p` is **irrelevant to `q`** in theory `t` if `p` is irrelevant
    to every rule in the theory whose head is `q` or `~q`. -/
def irrelevantToLiteral (t : Theory) (p q : Literal) : Prop :=
  (∀ r ∈ t.rules, r.head = q → irrelevant p r) ∧
  (∀ r ∈ t.rules, r.head = q.complement → irrelevant p r)

/-- `Rule.applicable` does not depend on irrelevant literals: if `p` and `~p`
    are both absent from `r.body`, then adding or removing `p` from the proved
    set does not change applicability. -/
theorem applicable_irrelevant (r : Rule) (proved : List Literal) (p : Literal)
    (h_irr : irrelevant p r) :
    r.applicable proved = r.applicable (proved.filter (· != p)) := by
  simp only [Rule.applicable]
  apply List.all_mem_congr
  intro l hl
  apply list_contains_iff_eq
  simp only [List.mem_filter]
  constructor
  · intro hm; exact ⟨hm, by simp; exact fun heq => h_irr.1 (heq ▸ hl)⟩
  · intro hm; exact hm.1

/-- `Rule.discarded` does not depend on irrelevant literals: if `p` and `~p`
    are both absent from `r.body`, then adding or removing `p` from the
    disproved set does not change whether `r` is discarded. -/
theorem discarded_irrelevant (r : Rule) (disproved : List Literal) (p : Literal)
    (h_irr : irrelevant p r) :
    r.discarded disproved = r.discarded (disproved.filter (· != p)) := by
  simp only [Rule.discarded]
  apply List.any_mem_congr
  intro l hl
  apply list_contains_iff_eq
  simp only [List.mem_filter]
  constructor
  · intro hm; exact ⟨hm, by simp; exact fun heq => h_irr.1 (heq ▸ hl)⟩
  · intro hm; exact hm.1

/-- **Ambiguity Locality (applicable form)**: if `p` is irrelevant to rule `r`
    (neither `p` nor `~p` in its body), then `r.applicable` gives the same
    result whether or not `p` is in the proved set.

    This is the core building block: ambiguous literals that don't appear in
    rule bodies cannot affect applicability, hence cannot affect provability. -/
theorem ambiguity_locality_applicable (r : Rule) (proved₁ proved₂ : List Literal) (p : Literal)
    (_h_irr : irrelevant p r)
    (h_agree : ∀ l ∈ r.body, (l ∈ proved₁ ↔ l ∈ proved₂)) :
    r.applicable proved₁ = r.applicable proved₂ := by
  simp only [Rule.applicable]
  apply List.all_mem_congr
  intro l hl
  exact list_contains_iff_eq (h_agree l hl)

/-- **Ambiguity Locality**: if `p` is irrelevant to literal `q` in theory `t`
    (neither `p` nor `~p` appears in any rule body for `q` or `~q`), and `q`
    is defeasibly provable (`+d`), then `q`'s provability does not depend on
    whether `p` is ambiguous.

    Specifically, the `canProve` check for `q` examines only rules for `q`
    and attackers for `~q`, none of which mention `p` or `~p`. Therefore,
    changing the status of `p` in the `+d` or `-d` sets cannot affect `q`'s
    derivation.

    Formalized as: `canProve` for `q` with sets `S` equals `canProve` for `q`
    with sets `S'` that differ only on `p` and `~p`. -/
theorem ambiguity_locality (t : Theory) (plusD plusd₁ plusd₂ minusd₁ minusd₂ : List Literal)
    (p q : Literal) (h_irr : irrelevantToLiteral t p q)
    (h_plusd_agree : ∀ l, l ≠ p → l ≠ p.complement → (l ∈ plusd₁ ↔ l ∈ plusd₂))
    (h_minusd_agree : ∀ l, l ≠ p → l ≠ p.complement → (l ∈ minusd₁ ↔ l ∈ minusd₂)) :
    canProve t plusD plusd₁ minusd₁ q = canProve t plusD plusd₂ minusd₂ q := by
  simp only [canProve]
  -- compNotDefinite doesn't depend on plusd/minusd at all
  -- hasApplicable: depends on plusd, but only through body literals of Rsd[q] rules
  -- allCountered: depends on plusd/minusd through body literals of R[~q] rules
  congr 1
  · congr 1
    · -- hasApplicable: same for both because p is irrelevant to all Rsd[q] rules
      apply List.any_mem_congr
      intro r hr
      have h_rsd := List.mem_filter.mp hr
      have h_head : r.head = q := by
        simp only [Bool.and_eq_true] at h_rsd
        exact LawfulBEq.eq_of_beq h_rsd.2.1
      have h_body_agree : ∀ l ∈ r.body, (l ∈ plusd₁ ↔ l ∈ plusd₂) := by
        intro l hl
        have h_p_irr := (h_irr.1 r h_rsd.1 h_head).1
        have h_pc_irr := (h_irr.1 r h_rsd.1 h_head).2
        have h_ne_p : l ≠ p := fun heq => h_p_irr (heq ▸ hl)
        have h_ne_pc : l ≠ p.complement := fun heq => h_pc_irr (heq ▸ hl)
        exact h_plusd_agree l h_ne_p h_ne_pc
      simp only [Rule.applicable]
      apply List.all_mem_congr
      intro l hl
      exact list_contains_iff_eq (h_body_agree l hl)
  · -- allCountered: same for both because p is irrelevant to all R[~q] rules
    apply List.all_mem_congr
    intro s hs
    -- s ∈ R[~q], so s.head = ~q
    have h_s_head : s.head = q.complement := by
      have := (List.mem_filter.mp hs).2
      exact LawfulBEq.eq_of_beq this
    simp only [attackerCountered]
    -- s.discarded: depends on minusd through body literals of s
    -- s.head.complement = q (since s.head = ~q and complement is involution)
    have h_s_irr : irrelevant p s := h_irr.2 s (List.mem_filter.mp hs).1 h_s_head
    congr 1
    · -- discarded
      simp only [Rule.discarded]
      apply List.any_mem_congr
      intro l hl
      have h_ne_p : l ≠ p := fun heq => h_s_irr.1 (heq ▸ hl)
      have h_ne_pc : l ≠ p.complement := fun heq => h_s_irr.2 (heq ▸ hl)
      exact list_contains_iff_eq (h_minusd_agree l h_ne_p h_ne_pc)
    · -- supporter applicable: supporters are in Rsd[s.head.complement] = Rsd[q]
      apply List.any_mem_congr
      intro supporter h_sup_mem
      congr 1
      -- supporter ∈ Rsd[s.head.complement]
      -- s.head = ~q, so s.head.complement = q
      rw [h_s_head, Literal.complement_involutive] at h_sup_mem
      have h_sup_rsd := List.mem_filter.mp h_sup_mem
      have h_sup_head : supporter.head = q := by
        simp only [Bool.and_eq_true] at h_sup_rsd
        exact LawfulBEq.eq_of_beq h_sup_rsd.2.1
      have h_sup_irr := h_irr.1 supporter h_sup_rsd.1 h_sup_head
      simp only [Rule.applicable]
      apply List.all_mem_congr
      intro l hl
      have h_ne_p : l ≠ p := fun heq => h_sup_irr.1 (heq ▸ hl)
      have h_ne_pc : l ≠ p.complement := fun heq => h_sup_irr.2 (heq ▸ hl)
      exact list_contains_iff_eq (h_plusd_agree l h_ne_p h_ne_pc)

end AmbiguityBlocking
