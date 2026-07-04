/-
  Spindle Trust: Weakest-Link Propagation

  Formalizes `TrustDerivationNode::weakest_link_trust` from
  `crates/spindle-core/src/trust.rs`: the trust degree of a conclusion is
  the minimum trust across its entire derivation tree.

    pub fn weakest_link_trust(&self) -> TrustValue {
        let mut min_trust = self.trust;
        for child in &self.children {
            min_trust = min_trust.min(child.weakest_link_trust());
        }
        min_trust
    }

  Key properties:
  - the chain degree never exceeds the root's own trust
    (weakestLink_le_trust) nor any child chain's degree
    (weakestLink_le_child) — a conclusion is never more trusted than any
    premise it rests on;
  - it is the greatest such lower bound (le_weakestLink);
  - it stays in the unit interval when all node trusts do
    (weakestLink_mem_unit).
-/
import Mathlib.Algebra.Order.Field.Rat
import Mathlib.Tactic.Linarith
import Spindle.Trust.Diminish

namespace Spindle.Trust

/-- A trust derivation tree: each node carries the trust of the rule/source
    that derived it, with children for the premises. Mirrors
    `TrustDerivationNode` (literal/source metadata omitted — they do not
    affect the degree computation). -/
inductive DerivationTree where
  | node (trust : TrustValue) (children : List DerivationTree)

namespace DerivationTree

/-- The root's own trust. -/
def trust : DerivationTree → TrustValue
  | node t _ => t

/-- The children of the root. -/
def children : DerivationTree → List DerivationTree
  | node _ cs => cs

/-- Weakest-link trust: minimum over the whole tree. Mirrors
    `weakest_link_trust`. -/
def weakestLink : DerivationTree → TrustValue
  | node t cs => go t cs
where
  /-- Fold the running minimum through the children, mirroring the Rust
      loop exactly. -/
  go (acc : TrustValue) : List DerivationTree → TrustValue
    | [] => acc
    | c :: cs => go (min acc (weakestLink c)) cs

/-! ## Generic facts about the accumulator fold -/

private theorem go_le_acc (cs : List DerivationTree) :
    ∀ acc, weakestLink.go acc cs ≤ acc := by
  induction cs with
  | nil => intro acc; exact le_refl acc
  | cons c cs ih =>
    intro acc
    calc weakestLink.go (min acc (weakestLink c)) cs
        ≤ min acc (weakestLink c) := ih _
    _ ≤ acc := min_le_left _ _

private theorem go_le_mem (cs : List DerivationTree) :
    ∀ acc, ∀ c ∈ cs, weakestLink.go acc cs ≤ weakestLink c := by
  induction cs with
  | nil => intro _ c hc; cases hc
  | cons x xs ih =>
    intro acc c hc
    rcases List.mem_cons.mp hc with rfl | hmem
    · calc weakestLink.go (min acc (weakestLink c)) xs
          ≤ min acc (weakestLink c) := go_le_acc xs _
      _ ≤ weakestLink c := min_le_right _ _
    · exact ih _ c hmem

private theorem le_go (cs : List DerivationTree) :
    ∀ acc b, b ≤ acc → (∀ c ∈ cs, b ≤ weakestLink c) →
      b ≤ weakestLink.go acc cs := by
  induction cs with
  | nil => intro acc b hb _; exact hb
  | cons x xs ih =>
    intro acc b hb h
    exact ih _ b (le_min hb (h x List.mem_cons_self))
      (fun c hc => h c (List.mem_cons_of_mem _ hc))

/-! ## Main properties -/

/-- The chain degree never exceeds the root's own trust: a conclusion is
    never more trusted than the rule that derived it. -/
theorem weakestLink_le_trust (t : TrustValue) (cs : List DerivationTree) :
    weakestLink (node t cs) ≤ t :=
  go_le_acc cs t

/-- The chain degree never exceeds any premise chain's degree: a conclusion
    is never more trusted than any premise it rests on. -/
theorem weakestLink_le_child (t : TrustValue) (cs : List DerivationTree)
    (c : DerivationTree) (hc : c ∈ cs) :
    weakestLink (node t cs) ≤ weakestLink c :=
  go_le_mem cs t c hc

/-- Weakest link is the GREATEST lower bound: any bound below the root
    trust and below every child chain is below the chain degree. -/
theorem le_weakestLink (t : TrustValue) (cs : List DerivationTree) (b : TrustValue)
    (hroot : b ≤ t) (hchildren : ∀ c ∈ cs, b ≤ weakestLink c) :
    b ≤ weakestLink (node t cs) :=
  le_go cs t b hroot hchildren

/-- If every node trust in the tree lies in [0, 1], so does the chain
    degree. Stated compositionally: the root trust is in range and every
    child chain is in range. -/
theorem weakestLink_mem_unit (t : TrustValue) (cs : List DerivationTree)
    (ht : 0 ≤ t ∧ t ≤ 1)
    (hcs : ∀ c ∈ cs, 0 ≤ weakestLink c ∧ weakestLink c ≤ 1) :
    0 ≤ weakestLink (node t cs) ∧ weakestLink (node t cs) ≤ 1 :=
  ⟨le_weakestLink t cs 0 ht.1 (fun c hc => (hcs c hc).1),
   le_trans (weakestLink_le_trust t cs) ht.2⟩

/-- Composition with diminishment: applying diminishers to a weakest-link
    degree keeps the final degree bounded by every link in the chain —
    the trust layer only ever lowers standing. -/
theorem diminishAll_weakestLink_le_child (t : TrustValue) (cs : List DerivationTree)
    (ds : List TrustValue) (c : DerivationTree) (hc : c ∈ cs)
    (h0 : 0 ≤ weakestLink (node t cs)) (hds : ∀ d ∈ ds, 0 ≤ d ∧ d ≤ 1) :
    diminishAll (weakestLink (node t cs)) ds ≤ weakestLink c := by
  have hle : diminishAll (weakestLink (node t cs)) ds ≤ weakestLink (node t cs) := by
    rw [diminishAll_eq_prod _ ds h0 hds]
    have hfac0 : ∀ a ∈ ds.map (fun d => 1 - d), (0:ℚ) ≤ a := by
      intro a ha
      simp only [List.mem_map] at ha
      obtain ⟨x, hx, rfl⟩ := ha
      linarith [(hds x hx).2]
    have hfac1 : ∀ a ∈ ds.map (fun d => 1 - d), a ≤ (1:ℚ) := by
      intro a ha
      simp only [List.mem_map] at ha
      obtain ⟨x, hx, rfl⟩ := ha
      linarith [(hds x hx).1]
    calc weakestLink (node t cs) * (ds.map (fun d => 1 - d)).prod
        ≤ weakestLink (node t cs) * 1 :=
          mul_le_mul_of_nonneg_left (list_prod_le_one _ hfac0 hfac1) h0
    _ = weakestLink (node t cs) := mul_one _
  exact le_trans hle (weakestLink_le_child t cs c hc)

end DerivationTree

end Spindle.Trust
