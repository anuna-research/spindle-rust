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

/-- Adding a non-cyclic superiority pair to an acyclic relation preserves acyclicity,
    provided the new pair doesn't create a cycle -/
theorem addSuperiority_preserves_acyclic (t : Theory) (w l : String)
    (hacyc : supAcyclic t)
    (hnocycle : ¬ supReachable t l w) :
    supAcyclic (t.addSuperiority w l) := by
  -- In the extended theory, any reachability chain either uses only old edges
  -- (contradicting hacyc) or uses the new edge (w,l), creating a path x->...->w->l->...->x
  -- which would mean l reaches w in the old theory (contradicting hnocycle).
  -- Full proof requires decomposing the reachability chain.
  sorry

end Properties
