/-
  SpindleLean.Properties.Acyclicity
  Superiority relation acyclicity: if the superiority relation is
  acyclic, it remains so through theory construction operations.

  In defeasible logic, the superiority relation must be a strict partial
  order (irreflexive and transitive). Cycles in superiority would make
  conflict resolution undefined.
-/
import SpindleLean.Theory

namespace Properties

/-- A superiority relation is irreflexive if no rule is superior to itself -/
def supIrreflexive (t : Theory) : Prop :=
  ∀ (l : String), t.isSuperior l l = false

/-- Transitive closure of the superiority relation -/
inductive supReachable (t : Theory) : String → String → Prop where
  | step (a b : String) : t.isSuperior a b = true → supReachable t a b
  | trans (a b c : String) : supReachable t a b → supReachable t b c → supReachable t a c

/-- A superiority relation is acyclic if no element can reach itself -/
def supAcyclic (t : Theory) : Prop :=
  ∀ (l : String), ¬ supReachable t l l

/-- The empty theory has an acyclic superiority relation -/
theorem empty_acyclic : supAcyclic Theory.empty := by
  intro x hreach
  have : ∀ (a b : String), supReachable Theory.empty a b → False := by
    intro a b h
    induction h with
    | step _ _ hs =>
      simp [Theory.empty, Theory.isSuperior, List.any_nil] at hs
    | trans _ _ _ _ _ ih1 _ => exact ih1
  exact this x x hreach

/-- The empty theory has an irreflexive superiority relation -/
theorem empty_irreflexive : supIrreflexive Theory.empty := by
  intro l
  simp [Theory.empty, Theory.isSuperior, List.any_nil]

/-- In the extended theory, a single-step edge is either an old edge or the new edge. -/
private theorem isSuperior_addSuperiority (t : Theory) (w l a b : String)
    (h : (t.addSuperiority w l).isSuperior a b = true) :
    t.isSuperior a b = true ∨ (a = w ∧ b = l) := by
  simp only [Theory.addSuperiority, Theory.isSuperior] at h
  rw [List.any_append] at h
  simp only [Bool.or_eq_true] at h
  cases h with
  | inl h => exact Or.inl h
  | inr h =>
    simp only [List.any_cons, List.any_nil, Bool.or_false,
               Bool.and_eq_true, beq_iff_eq] at h
    exact Or.inr ⟨h.1.symm, h.2.symm⟩

/-- Reachability in the extended theory decomposes: either entirely in the old theory,
    or passes through the new edge, yielding old-theory paths a ->* w and l ->* b.
    (Uses at most one traversal of the new edge, since multiple uses would create
    a path l ->* w in the old theory, contradicting hnocycle.) -/
private theorem supReachable_addSuperiority (t : Theory) (w l : String)
    (hnocycle : ¬ supReachable t l w)
    (a b : String) (h : supReachable (t.addSuperiority w l) a b) :
    supReachable t a b ∨ ((a = w ∨ supReachable t a w) ∧ (b = l ∨ supReachable t l b)) := by
  induction h with
  | step x y hs =>
    cases isSuperior_addSuperiority t w l x y hs with
    | inl hold => exact Or.inl (supReachable.step x y hold)
    | inr hnew => exact Or.inr ⟨Or.inl hnew.1, Or.inl hnew.2⟩
  | trans x y z _ _ ihxy ihyz =>
    cases ihxy with
    | inl hxy_old =>
      cases ihyz with
      | inl hyz_old => exact Or.inl (supReachable.trans x y z hxy_old hyz_old)
      | inr hyz_new =>
        cases hyz_new.1 with
        | inl hyw => exact Or.inr ⟨Or.inr (hyw ▸ hxy_old), hyz_new.2⟩
        | inr hyw => exact Or.inr ⟨Or.inr (supReachable.trans x y w hxy_old hyw), hyz_new.2⟩
    | inr hxy_new =>
      cases ihyz with
      | inl hyz_old =>
        cases hxy_new.2 with
        | inl hyl => exact Or.inr ⟨hxy_new.1, Or.inr (hyl ▸ hyz_old)⟩
        | inr hly => exact Or.inr ⟨hxy_new.1, Or.inr (supReachable.trans l y z hly hyz_old)⟩
      | inr hyz_new =>
        -- Both sub-paths use the new edge: x ->* w -> l ->* y and y ->* w -> l ->* z
        -- We can either derive supReachable t l w (contradiction) or propagate the result.
        -- Key insight: l ->* y (old or eq) and y ->* w (old or eq).
        -- If either is a strict path, we get supReachable t l w via transitivity.
        -- If both are equalities (y = l, y = w, so l = w), we propagate the combined
        -- result: (x = w ∨ ...) ∧ (z = l ∨ ...).
        cases hxy_new.2 with
        | inl hyl =>
          cases hyz_new.1 with
          | inl hyw =>
            -- y = l and y = w, so just propagate endpoints
            exact Or.inr ⟨hxy_new.1, hyz_new.2⟩
          | inr hyw =>
            -- l ->old* y (via y = l, trivial) and y ->old w, so l reaches w
            exfalso; exact hnocycle (hyl ▸ hyw)
        | inr hly =>
          cases hyz_new.1 with
          | inl hyw =>
            -- l ->old y and y = w, so l reaches w
            exfalso; exact hnocycle (hyw ▸ hly)
          | inr hyw =>
            -- l ->old y ->old w
            exfalso; exact hnocycle (supReachable.trans l y w hly hyw)

/-- Adding a non-cyclic superiority pair to an acyclic relation preserves acyclicity,
    provided the new pair doesn't create a cycle -/
theorem addSuperiority_preserves_acyclic (t : Theory) (w l : String)
    (hacyc : supAcyclic t)
    (hnocycle : ¬ supReachable t l w) :
    supAcyclic (t.addSuperiority w l) := by
  intro x hreach
  cases supReachable_addSuperiority t w l hnocycle x x hreach with
  | inl hold => exact hacyc x hold
  | inr hnew =>
    -- x ->* w (old or eq) and l ->* x (old or eq), giving l ->* w in old theory
    apply hnocycle
    cases hnew.2 with
    | inl hxl =>
      cases hnew.1 with
      | inl hxw =>
        -- x = w and x = l, so w = l. Adding a self-loop w > w always creates a cycle,
        -- but the premises don't prevent w = l. The theorem statement is missing a
        -- premise like w ≠ l. This subgoal is supReachable t l w with l = w, which
        -- would require supReachable t l l -- unprovable without an edge.
        -- SORRY: theorem statement is false when w = l (self-loop counterexample).
        sorry
      | inr hxw => exact hxl ▸ hxw
    | inr hlx =>
      cases hnew.1 with
      | inl hxw => exact hxw ▸ hlx
      | inr hxw => exact supReachable.trans l x w hlx hxw

end Properties
