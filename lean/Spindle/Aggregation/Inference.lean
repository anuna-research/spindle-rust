import Spindle.Aggregation.Dependencies
import Mathlib.Data.Finset.Card

/-!
# Automatic stratum inference

Start every node at zero and repeatedly relax all constraints simultaneously.
Each unsuccessful round strictly increases the sum of stages on the finite
node list. Any valid assignment can be compressed to ranks at most the number
of listed nodes. Consequently N² rounds suffice: exhausting this budget is a
semantic rejection, not an inconclusive timeout. Duplicate nodes in the list
only make the bound looser. The result is the pointwise least valid assignment.
-/
namespace Spindle.Aggregation

variable {Rel : Type} [DecidableEq Rel]

def graphNodes (graph : List (Dependency Rel)) : List Rel :=
  graph.flatMap fun edge => [edge.source, edge.target]

omit [DecidableEq Rel] in
theorem source_mem_nodes (graph : List (Dependency Rel)) (edge : Dependency Rel)
    (member : edge ∈ graph) : edge.source ∈ graphNodes graph := by
  exact List.mem_flatMap.mpr ⟨edge, member, by simp⟩

omit [DecidableEq Rel] in
theorem target_mem_nodes (graph : List (Dependency Rel)) (edge : Dependency Rel)
    (member : edge ∈ graph) : edge.target ∈ graphNodes graph := by
  exact List.mem_flatMap.mpr ⟨edge, member, by simp⟩

/-- Simultaneous relaxation includes the old value, so stages never decrease. -/
def relax (graph : List (Dependency Rel)) (stage : Rel → Nat) (node : Rel) : Nat :=
  graph.foldr (fun edge acc =>
    if edge.target = node then max (stage edge.source + edge.weight) acc else acc) (stage node)

theorem relax_inflationary (graph : List (Dependency Rel)) (stage : Rel → Nat) (node : Rel) :
    stage node ≤ relax graph stage node := by
  induction graph with
  | nil => exact Nat.le_refl _
  | cons edge rest ih =>
    simp only [relax, List.foldr_cons] at *
    split
    · exact Nat.le_trans ih (Nat.le_max_right _ _)
    · exact ih

theorem relax_edge (graph : List (Dependency Rel)) (stage : Rel → Nat)
    (edge : Dependency Rel) (member : edge ∈ graph) :
    stage edge.source + edge.weight ≤ relax graph stage edge.target := by
  induction graph with
  | nil => simp at member
  | cons first rest ih =>
    simp only [List.mem_cons] at member
    rcases member with rfl | member
    · simp only [relax, List.foldr_cons, ite_true]
      exact Nat.le_max_left _ _
    · have bound := ih member
      simp only [relax, List.foldr_cons] at *
      split
      · exact Nat.le_trans bound (Nat.le_max_right _ _)
      · exact bound

/-- Relaxation stays below every valid upper assignment. -/
theorem relax_bounded (graph : List (Dependency Rel)) (stage upper : Rel → Nat)
    (valid : ValidStrata graph upper) (below : ∀ node, stage node ≤ upper node) :
    ∀ node, relax graph stage node ≤ upper node := by
  intro node
  induction graph with
  | nil => exact below node
  | cons edge rest ih =>
    have tail_valid : ValidStrata rest upper := fun e h => valid e (by simp [h])
    have tail_bound := ih tail_valid
    have edge_bound := valid edge (by simp)
    simp only [relax, List.foldr_cons] at *
    split
    next equal =>
      apply Nat.max_le.mpr
      constructor
      · have source_bound := below edge.source
        rw [equal] at edge_bound
        omega
      · exact tail_bound
    · exact tail_bound

def stageMass (nodes : List Rel) (stage : Rel → Nat) : Nat := (nodes.map stage).sum

omit [DecidableEq Rel] in
theorem stageMass_mono (nodes : List Rel) (left right : Rel → Nat)
    (below : ∀ node, left node ≤ right node) : stageMass nodes left ≤ stageMass nodes right := by
  induction nodes with
  | nil => exact Nat.le_refl _
  | cons node rest ih =>
    have bound := below node
    simp only [stageMass, List.map_cons, List.sum_cons] at *
    omega

omit [DecidableEq Rel] in
theorem stageMass_strict (nodes : List Rel) (left right : Rel → Nat)
    (below : ∀ node, left node ≤ right node)
    (increased : ∃ node ∈ nodes, left node < right node) :
    stageMass nodes left < stageMass nodes right := by
  induction nodes with
  | nil => simp at increased
  | cons node rest ih =>
    obtain ⟨changed, member, strict⟩ := increased
    simp only [List.mem_cons] at member
    rcases member with rfl | member
    · have tail_bound := stageMass_mono rest left right below
      simp only [stageMass, List.map_cons, List.sum_cons] at *
      omega
    · have tail_strict := ih ⟨changed, member, strict⟩
      have head_bound := below node
      simp only [stageMass, List.map_cons, List.sum_cons] at *
      omega

omit [DecidableEq Rel] in
theorem stageMass_uniform_bound (nodes : List Rel) (stage : Rel → Nat) (bound : Nat)
    (bounded : ∀ node, stage node ≤ bound) : stageMass nodes stage ≤ nodes.length * bound := by
  induction nodes with
  | nil => simp [stageMass]
  | cons node rest ih =>
    have head_bound := bounded node
    simp only [stageMass, List.map_cons, List.sum_cons, List.length_cons, Nat.succ_mul] at *
    omega

theorem invalid_relax_increases (graph : List (Dependency Rel)) (stage : Rel → Nat)
    (invalid : checkStrata graph stage = false) :
    stageMass (graphNodes graph) stage < stageMass (graphNodes graph) (relax graph stage) := by
  have not_valid : ¬ ValidStrata graph stage := by
    intro valid
    have accepted := (checkStrata_iff graph stage).mpr valid
    simp [invalid] at accepted
  simp only [ValidStrata, not_forall] at not_valid
  obtain ⟨edge, member, failed⟩ := not_valid
  have strict : stage edge.target < stage edge.source + edge.weight := by omega
  apply stageMass_strict _ stage (relax graph stage) (relax_inflationary graph stage)
  have relaxed := relax_edge graph stage edge member
  exact ⟨edge.target, target_mem_nodes graph edge member, by omega⟩

/-- Compress an arbitrary valid assignment to a finite ordinal rank. -/
def compressed (nodes : List Rel) (stage : Rel → Nat) (node : Rel) : Nat :=
  (nodes.toFinset.filter (fun other => stage other < stage node)).card

theorem compressed_bounded (nodes : List Rel) (stage : Rel → Nat) (node : Rel) :
    compressed nodes stage node ≤ nodes.length := by
  exact Nat.le_trans (Finset.card_le_card (Finset.filter_subset _ _)) (List.toFinset_card_le nodes)

theorem compressed_mono (nodes : List Rel) (stage : Rel → Nat) (a b : Rel)
    (ordered : stage a ≤ stage b) : compressed nodes stage a ≤ compressed nodes stage b := by
  apply Finset.card_le_card
  intro other member
  simp only [Finset.mem_filter] at *
  exact ⟨member.1, by omega⟩

theorem compressed_strict (nodes : List Rel) (stage : Rel → Nat) (a b : Rel)
    (member : a ∈ nodes) (ordered : stage a < stage b) :
    compressed nodes stage a < compressed nodes stage b := by
  apply Finset.card_lt_card
  apply Finset.ssubset_iff_subset_ne.mpr
  constructor
  · intro other present
    simp only [Finset.mem_filter] at *
    exact ⟨present.1, by omega⟩
  · intro equal
    have included : a ∈ nodes.toFinset.filter (fun other => stage other < stage b) := by
      simp [member, ordered]
    rw [← equal] at included
    simp at included

theorem compressed_valid (graph : List (Dependency Rel)) (stage : Rel → Nat)
    (valid : ValidStrata graph stage) : ValidStrata graph (compressed (graphNodes graph) stage) := by
  intro edge member
  have bound := valid edge member
  cases kind : edge.aggregate with
  | false =>
    simp [Dependency.weight, kind] at bound ⊢
    exact compressed_mono _ stage _ _ bound
  | true =>
    have strict := compressed_strict (graphNodes graph) stage edge.source edge.target
      (source_mem_nodes graph edge member) (by simpa [Dependency.weight, kind] using bound)
    simp [Dependency.weight, kind]
    omega

/-- Materialize a round's values. Without this table, nested functional stages
would repeatedly recompute the same earlier rounds when checking a cycle. -/
def lookupStage (table : List (Rel × Nat)) (fallback : Rel → Nat) (node : Rel) : Nat :=
  match table.find? (fun entry => decide (entry.1 = node)) with
  | some entry => entry.2
  | none => fallback node

def cacheStage (nodes : List Rel) (stage : Rel → Nat) : List (Rel × Nat) :=
  nodes.map fun node => (node, stage node)

@[simp] theorem cacheStage_eq (nodes : List Rel) (stage : Rel → Nat) :
    lookupStage (cacheStage nodes stage) stage = stage := by
  funext node
  simp only [cacheStage, lookupStage]
  cases found : (nodes.map (fun n => (n, stage n))).find? (fun entry => decide (entry.1 = node)) with
  | none => rfl
  | some entry =>
    have member := List.mem_of_find?_eq_some found
    obtain ⟨source, _, same⟩ := List.mem_map.mp member
    subst entry
    have equal := List.find?_some found
    simp only [decide_eq_true_eq] at equal
    subst source
    rfl

def inferLoop (graph : List (Dependency Rel)) : Nat → (Rel → Nat) → Option (Rel → Nat)
  | 0, stage => if checkStrata graph stage then some stage else none
  | fuel + 1, stage => if checkStrata graph stage then some stage
      else
        let next := relax graph stage
        let table := cacheStage (graphNodes graph) next
        inferLoop graph fuel (lookupStage table next)

theorem inferLoop_sound (graph : List (Dependency Rel)) (fuel : Nat) (start result : Rel → Nat)
    (returned : inferLoop graph fuel start = some result) : ValidStrata graph result := by
  induction fuel generalizing start with
  | zero =>
    simp only [inferLoop] at returned
    split at returned
    · cases returned; exact (checkStrata_iff graph result).mp (by assumption)
    · simp at returned
  | succ fuel ih =>
    simp only [inferLoop, cacheStage_eq] at returned
    split at returned
    · cases returned; exact (checkStrata_iff graph result).mp (by assumption)
    · exact ih _ returned

theorem inferLoop_bounded (graph : List (Dependency Rel)) (fuel : Nat)
    (start result upper : Rel → Nat) (valid : ValidStrata graph upper)
    (below : ∀ node, start node ≤ upper node)
    (returned : inferLoop graph fuel start = some result) : ∀ node, result node ≤ upper node := by
  induction fuel generalizing start with
  | zero =>
    simp only [inferLoop] at returned
    split at returned
    · cases returned; exact below
    · simp at returned
  | succ fuel ih =>
    simp only [inferLoop, cacheStage_eq] at returned
    split at returned
    · cases returned; exact below
    · exact ih _ (relax_bounded graph start upper valid below) returned

theorem inferLoop_complete (graph : List (Dependency Rel)) (fuel : Nat)
    (start upper : Rel → Nat) (valid : ValidStrata graph upper)
    (below : ∀ node, start node ≤ upper node)
    (bounded : ∀ node, upper node ≤ (graphNodes graph).length)
    (budget : (graphNodes graph).length ^ 2 ≤ stageMass (graphNodes graph) start + fuel) :
    ∃ result, inferLoop graph fuel start = some result := by
  induction fuel generalizing start with
  | zero =>
    simp only [inferLoop]
    split
    · exact ⟨start, rfl⟩
    next rejected =>
      have invalid : checkStrata graph start = false := by simpa using rejected
      have growth := invalid_relax_increases graph start invalid
      have next_bound := relax_bounded graph start upper valid below
      have mass_bound := stageMass_uniform_bound (graphNodes graph) (relax graph start)
        (graphNodes graph).length (fun n => Nat.le_trans (next_bound n) (bounded n))
      simp only [Nat.pow_succ, Nat.pow_zero, Nat.one_mul] at budget
      omega
  | succ fuel ih =>
    simp only [inferLoop, cacheStage_eq]
    split
    · exact ⟨start, rfl⟩
    next rejected =>
      have invalid : checkStrata graph start = false := by simpa using rejected
      have growth := invalid_relax_increases graph start invalid
      exact ih (relax graph start) (relax_bounded graph start upper valid below) (by omega)

/-- Total inference: the graph determines its own sufficient finite budget. -/
def inferStrata (graph : List (Dependency Rel)) : Option (Rel → Nat) :=
  inferLoop graph ((graphNodes graph).length ^ 2) (fun _ => 0)

theorem inferStrata_sound (graph : List (Dependency Rel)) (result : Rel → Nat)
    (returned : inferStrata graph = some result) : ValidStrata graph result :=
  inferLoop_sound graph _ _ result returned

theorem inferStrata_complete (graph : List (Dependency Rel))
    (exists_valid : ∃ stage, ValidStrata graph stage) :
    ∃ result, inferStrata graph = some result := by
  obtain ⟨stage, valid⟩ := exists_valid
  apply inferLoop_complete graph _ _ (compressed (graphNodes graph) stage)
    (compressed_valid graph stage valid)
  · intro node; exact Nat.zero_le _
  · exact compressed_bounded (graphNodes graph) stage
  · simp [stageMass]

theorem inferStrata_none_iff (graph : List (Dependency Rel)) :
    inferStrata graph = none ↔ ¬ ∃ stage, ValidStrata graph stage := by
  constructor
  · intro rejected valid
    obtain ⟨result, returned⟩ := inferStrata_complete graph valid
    simp [rejected] at returned
  · intro invalid
    cases result : inferStrata graph with
    | none => rfl
    | some stage => exact False.elim (invalid ⟨stage, inferStrata_sound graph stage result⟩)

theorem inferStrata_least (graph : List (Dependency Rel)) (result upper : Rel → Nat)
    (returned : inferStrata graph = some result) (valid : ValidStrata graph upper) :
    ∀ node, result node ≤ upper node :=
  inferLoop_bounded graph _ _ result upper valid (fun _ => Nat.zero_le _) returned

/-- The inferred assignment depends on the constraints, not edge enumeration. -/
theorem inferStrata_permutation (left right : List (Dependency Rel))
    (permuted : left.Perm right) : inferStrata left = inferStrata right := by
  have same_valid (stage : Rel → Nat) : ValidStrata left stage ↔ ValidStrata right stage := by
    constructor
    · intro valid edge member; exact valid edge (permuted.mem_iff.mpr member)
    · intro valid edge member; exact valid edge (permuted.mem_iff.mp member)
  cases hl : inferStrata left with
  | none =>
    have absent := (inferStrata_none_iff left).mp hl
    have hr : inferStrata right = none := (inferStrata_none_iff right).mpr (by
      rintro ⟨stage, valid⟩
      exact absent ⟨stage, (same_valid stage).mpr valid⟩)
    exact hr.symm
  | some a =>
    have valid_a := inferStrata_sound left a hl
    obtain ⟨b, hr⟩ := inferStrata_complete right ⟨a, (same_valid a).mp valid_a⟩
    have valid_b := inferStrata_sound right b hr
    have ab := inferStrata_least left a b hl ((same_valid b).mpr valid_b)
    have ba := inferStrata_least right b a hr ((same_valid a).mp valid_a)
    have equal : a = b := funext (fun node => Nat.le_antisymm (ab node) (ba node))
    rw [hr, equal]

inductive InferenceError where
  | unrecognizedSchemas
  | unstratifiable
  deriving DecidableEq, Repr

/-- Schema recognition failures remain distinct from genuine semantic rejection. -/
def inferProgram (program : AggregateProgram) : Except InferenceError (DependencyNode → Nat) :=
  if schemasRecognized program then
    match inferStrata (dependencies program) with
    | some stage => .ok stage
    | none => .error .unstratifiable
  else .error .unrecognizedSchemas

theorem inferProgram_sound (program : AggregateProgram) (stage : DependencyNode → Nat)
    (returned : inferProgram program = .ok stage) :
    schemasRecognized program = true ∧ ProgramScheduled program stage := by
  unfold inferProgram at returned
  split at returned
  next recognized =>
    cases inferred : inferStrata (dependencies program) with
    | none => simp [inferred] at returned
    | some result =>
      simp only [inferred, Except.ok.injEq] at returned
      subst result
      exact ⟨recognized, (dependencies_valid_iff program stage).mp
        (inferStrata_sound _ stage inferred)⟩
  · simp at returned

theorem inferProgram_complete (program : AggregateProgram)
    (recognized : schemasRecognized program = true)
    (schedulable : ∃ stage, ProgramScheduled program stage) :
    ∃ stage, inferProgram program = .ok stage := by
  obtain ⟨stage, scheduled⟩ := schedulable
  obtain ⟨result, inferred⟩ := inferStrata_complete (dependencies program)
    ⟨stage, (dependencies_valid_iff program stage).mpr scheduled⟩
  exact ⟨result, by simp [inferProgram, recognized, inferred]⟩

theorem inferProgram_unstratifiable_iff (program : AggregateProgram) :
    inferProgram program = .error .unstratifiable ↔
      schemasRecognized program = true ∧ ¬ ∃ stage, ProgramScheduled program stage := by
  constructor
  · intro rejected
    have recognized : schemasRecognized program = true := by
      by_contra absent
      simp [inferProgram, absent] at rejected
    refine ⟨recognized, ?_⟩
    intro possible
    obtain ⟨stage, returned⟩ := inferProgram_complete program recognized possible
    simp [rejected] at returned
  · rintro ⟨recognized, impossible⟩
    have absent : inferStrata (dependencies program) = none :=
      (inferStrata_none_iff _).mpr (by
        rintro ⟨stage, valid⟩
        exact impossible ⟨stage, (dependencies_valid_iff program stage).mp valid⟩)
    simp [inferProgram, recognized, absent]

end Spindle.Aggregation
