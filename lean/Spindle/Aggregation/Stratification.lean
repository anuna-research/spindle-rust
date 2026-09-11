import Mathlib.Data.List.Basic
import Lean.Elab.Tactic.Omega

/-!
# Dependency ordering for aggregation

A relation denotes the complete conflict domain, including complementary heads.
Every producer of that domain is assigned the same stratum. Ordinary dependencies
have weight zero; aggregate dependencies have weight one. This file checks a
supplied assignment; it does not yet implement an SCC/stratum inference algorithm.
-/
namespace Spindle.Aggregation

structure Dependency (Rel : Type) where
  source : Rel
  target : Rel
  aggregate : Bool

def Dependency.weight {Rel : Type} (edge : Dependency Rel) : Nat :=
  if edge.aggregate then 1 else 0

variable {Rel : Type}

/-- Ordinary inputs must be available; aggregate inputs must be finalized. -/
def ValidStrata (graph : List (Dependency Rel)) (stratum : Rel → Nat) : Prop :=
  ∀ edge ∈ graph, stratum edge.source + edge.weight ≤ stratum edge.target

/-- Paths include ordinary edges: omitting them misses indirect fold cycles. -/
inductive DependencyPath (graph : List (Dependency Rel)) : Rel → Rel → Nat → Prop
  | nil (source : Rel) : DependencyPath graph source source 0
  | cons {edge : Dependency Rel} {target : Rel} {weight : Nat}
      (member : edge ∈ graph) (rest : DependencyPath graph edge.target target weight) :
      DependencyPath graph edge.source target (edge.weight + weight)

theorem path_bound {graph : List (Dependency Rel)} {stratum : Rel → Nat}
    (valid : ValidStrata graph stratum) {source target : Rel} {weight : Nat}
    (path : DependencyPath graph source target weight) :
    stratum source + weight ≤ stratum target := by
  induction path with
  | nil => omega
  | cons member _ ih =>
    have edge_bound := valid _ member
    omega

theorem no_aggregate_cycle {graph : List (Dependency Rel)} {stratum : Rel → Nat}
    (valid : ValidStrata graph stratum) {source : Rel} {weight : Nat}
    (path : DependencyPath graph source source weight) : weight = 0 := by
  have bound := path_bound valid path
  omega

theorem aggregate_input_finalized {graph : List (Dependency Rel)}
    {stratum : Rel → Nat} (valid : ValidStrata graph stratum)
    {edge : Dependency Rel} (member : edge ∈ graph) (aggregate : edge.aggregate = true) :
    stratum edge.source < stratum edge.target := by
  have bound := valid edge member
  simp [Dependency.weight, aggregate] at bound
  omega

/-- A frozen view excludes rows belonging to the current or any later stratum. -/
def frozenRows {Row : Type} (owner : Row → Rel) (stratum : Rel → Nat)
    (consumer : Nat) (rows : List Row) : List Row :=
  rows.filter (fun row => decide (stratum (owner row) < consumer))

theorem frozenRows_append_future {Row : Type} (owner : Row → Rel)
    (stratum : Rel → Nat) (consumer : Nat) (past future : List Row)
    (later : ∀ row ∈ future, consumer ≤ stratum (owner row)) :
    frozenRows owner stratum consumer (past ++ future) =
      frozenRows owner stratum consumer past := by
  have empty : future.filter (fun row => decide (stratum (owner row) < consumer)) = [] := by
    apply List.filter_eq_nil_iff.mpr
    intro row member
    simp only [Bool.not_eq_true, decide_eq_false_iff_not]
    have bound := later row member
    omega
  simp [frozenRows, List.filter_append, empty]

-- The PR's missed case: A --fold--> B --ordinary--> A is invalid.
example : ¬ ∃ s : Nat → Nat, ValidStrata [⟨0, 1, true⟩, ⟨1, 0, false⟩] s := by
  rintro ⟨s, valid⟩
  have forward := valid ⟨0, 1, true⟩ (by simp)
  have backward := valid ⟨1, 0, false⟩ (by simp)
  simp [Dependency.weight] at forward backward
  omega

-- Positive recursion stays legal.
example : ValidStrata [⟨0, 1, false⟩, ⟨1, 0, false⟩] (fun _ : Nat => 0) := by
  intro edge member
  simp at member
  rcases member with rfl | rfl <;> decide

end Spindle.Aggregation
