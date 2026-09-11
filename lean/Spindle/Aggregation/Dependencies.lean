import Spindle.Aggregation.Syntax
import Spindle.Aggregation.Stratification

/-!
# Extracting a complete dependency graph for the schema fragment

Rules have their own nodes. Bidirectional zero-weight edges connect a rule to
every head domain, so all producers, opposing heads, defeaters, and heads of a
multi-head rule are finalized together. Every relational body occurrence adds
an edge to the rule: weight zero for ordinary premises, one for folds.

Priorities do not create extra dependencies. In SDL they matter only for
competing heads, which already share a domain. Adding edges for unrelated
priority pairs would reject programs for relationships with no semantic effect.
-/
namespace Spindle.Aggregation

inductive DependencyNode where
  | predicate (key : PredicateKey)
  | rule (label : String)
  deriving DecidableEq, Repr

abbrev SchemaGraph := List (Dependency DependencyNode)

def headEdges (rule : SchemaRule) : SchemaGraph :=
  rule.heads.flatMap fun head =>
    [⟨.rule rule.label, .predicate head.key, false⟩,
     ⟨.predicate head.key, .rule rule.label, false⟩]

def bodyEdges (rule : SchemaRule) : SchemaGraph :=
  rule.body.filterMap fun condition => condition.input.map fun (pattern, aggregate) =>
    ⟨.predicate pattern.key, .rule rule.label, aggregate⟩

def ruleEdges (rule : SchemaRule) : SchemaGraph := headEdges rule ++ bodyEdges rule

def dependencies (program : AggregateProgram) : SchemaGraph :=
  program.rules.flatMap ruleEdges

/-- Relational constraints stated independently of graph construction. -/
def RuleScheduled (rule : SchemaRule) (stage : DependencyNode → Nat) : Prop :=
  (∀ head ∈ rule.heads, stage (.rule rule.label) = stage (.predicate head.key)) ∧
  (∀ condition ∈ rule.body, ∀ pattern aggregate,
    condition.input = some (pattern, aggregate) →
    stage (.predicate pattern.key) + (if aggregate then 1 else 0) ≤ stage (.rule rule.label))

def ProgramScheduled (program : AggregateProgram) (stage : DependencyNode → Nat) : Prop :=
  ∀ rule ∈ program.rules, RuleScheduled rule stage

theorem head_edges_valid_iff (rule : SchemaRule) (stage : DependencyNode → Nat) :
    ValidStrata (headEdges rule) stage ↔
      ∀ head ∈ rule.heads, stage (.rule rule.label) = stage (.predicate head.key) := by
  constructor
  · intro valid head member
    have forward := valid ⟨.rule rule.label, .predicate head.key, false⟩ (by
      simp only [headEdges, List.mem_flatMap]
      exact ⟨head, member, by simp⟩)
    have backward := valid ⟨.predicate head.key, .rule rule.label, false⟩ (by
      simp only [headEdges, List.mem_flatMap]
      exact ⟨head, member, by simp⟩)
    simp [Dependency.weight] at forward backward
    omega
  · intro aligned edge member
    obtain ⟨head, member, edges⟩ := List.mem_flatMap.mp member
    have equal := aligned head member
    simp at edges
    rcases edges with rfl | rfl <;> simp [Dependency.weight, equal]

theorem body_edges_valid_iff (rule : SchemaRule) (stage : DependencyNode → Nat) :
    ValidStrata (bodyEdges rule) stage ↔
      ∀ condition ∈ rule.body, ∀ pattern aggregate,
        condition.input = some (pattern, aggregate) →
        stage (.predicate pattern.key) + (if aggregate then 1 else 0) ≤
          stage (.rule rule.label) := by
  constructor
  · intro valid condition member pattern aggregate input
    apply valid ⟨.predicate pattern.key, .rule rule.label, aggregate⟩
    apply List.mem_filterMap.mpr
    exact ⟨condition, member, by simp [input]⟩
  · intro ordered edge member
    obtain ⟨condition, condition_member, mapped⟩ := List.mem_filterMap.mp member
    cases input : condition.input with
    | none => simp [input] at mapped
    | some pair =>
      obtain ⟨pattern, aggregate⟩ := pair
      simp [input] at mapped
      subst edge
      exact ordered condition condition_member pattern aggregate input

theorem rule_edges_valid_iff (rule : SchemaRule) (stage : DependencyNode → Nat) :
    ValidStrata (ruleEdges rule) stage ↔ RuleScheduled rule stage := by
  have split : ValidStrata (ruleEdges rule) stage ↔
      ValidStrata (headEdges rule) stage ∧ ValidStrata (bodyEdges rule) stage := by
    simp only [ValidStrata, ruleEdges, List.mem_append, or_imp, forall_and]
  rw [split, head_edges_valid_iff, body_edges_valid_iff]
  rfl

/-- Soundness and completeness of extraction relative to the schema constraints.
No body occurrence or producer can be silently omitted. -/
theorem dependencies_valid_iff (program : AggregateProgram) (stage : DependencyNode → Nat) :
    ValidStrata (dependencies program) stage ↔ ProgramScheduled program stage := by
  constructor
  · intro valid rule member
    apply (rule_edges_valid_iff rule stage).mp
    intro edge edge_member
    exact valid edge (List.mem_flatMap.mpr ⟨rule, member, edge_member⟩)
  · intro scheduled edge member
    obtain ⟨rule, member, edge_member⟩ := List.mem_flatMap.mp member
    exact (rule_edges_valid_iff rule stage).mpr (scheduled rule member) edge edge_member

/-- In particular a fold waits for every producer of its pattern's domain,
including a defeater or complementary/modal head sharing that domain. -/
theorem fold_waits_for_producer (program : AggregateProgram) (stage : DependencyNode → Nat)
    (valid : ValidStrata (dependencies program) stage)
    (consumer producer : SchemaRule) (hc : consumer ∈ program.rules) (hp : producer ∈ program.rules)
    (fold : FoldExpression) (hf : Condition.fold fold ∈ consumer.body)
    (head : Pattern) (hh : head ∈ producer.heads) (same : head.key = fold.pattern.key) :
    stage (.rule producer.label) < stage (.rule consumer.label) := by
  have scheduled := (dependencies_valid_iff program stage).mp valid
  have produced := (scheduled producer hp).1 head hh
  have consumed := (scheduled consumer hc).2 (.fold fold) hf fold.pattern true rfl
  simp only [ite_true] at consumed
  rw [same] at produced
  omega

theorem producers_same_stage (program : AggregateProgram) (stage : DependencyNode → Nat)
    (valid : ValidStrata (dependencies program) stage)
    (left right : SchemaRule) (hl : left ∈ program.rules) (hr : right ∈ program.rules)
    (a b : Pattern) (ha : a ∈ left.heads) (hb : b ∈ right.heads) (same : a.key = b.key) :
    stage (.rule left.label) = stage (.rule right.label) := by
  have scheduled := (dependencies_valid_iff program stage).mp valid
  rw [(scheduled left hl).1 a ha, (scheduled right hr).1 b hb, same]

/-- Executable certificate checking, with no unchecked success path. -/
def checkStrata {Rel : Type} (graph : List (Dependency Rel)) (stage : Rel → Nat) : Bool :=
  graph.all fun edge => decide (stage edge.source + edge.weight ≤ stage edge.target)

theorem checkStrata_iff {Rel : Type} (graph : List (Dependency Rel)) (stage : Rel → Nat) :
    checkStrata graph stage = true ↔ ValidStrata graph stage := by
  simp [checkStrata, ValidStrata]

theorem checkProgram_iff (program : AggregateProgram) (stage : DependencyNode → Nat) :
    checkStrata (dependencies program) stage = true ↔ ProgramScheduled program stage := by
  rw [checkStrata_iff, dependencies_valid_iff]

/-- Scope validation for this typed fragment. Other checks (expression types,
range restriction, superiority acyclicity) belong to their respective layers. -/
def schemasRecognized (program : AggregateProgram) : Bool :=
  program.rules.all SchemaRule.hasStaticNames &&
    decide (program.rules.map SchemaRule.label).Nodup &&
    program.priorities.all (fun (winner, loser) =>
      program.rules.any (fun rule => rule.label == winner) &&
      program.rules.any (fun rule => rule.label == loser))

/-- Only expose a graph after static-name, unique-label and reference checks. -/
def extractDependencies (program : AggregateProgram) : Option SchemaGraph :=
  if schemasRecognized program then some (dependencies program) else none

theorem extractDependencies_iff (program : AggregateProgram) (graph : SchemaGraph) :
    extractDependencies program = some graph ↔
      schemasRecognized program = true ∧ dependencies program = graph := by
  simp only [extractDependencies]
  split <;> simp_all

/-- A recognized, accepted certificate establishes all schema-level constraints. -/
theorem extracted_certificate_sound (program : AggregateProgram) (graph : SchemaGraph)
    (extracted : extractDependencies program = some graph) (stage : DependencyNode → Nat)
    (accepted : checkStrata graph stage = true) : ProgramScheduled program stage := by
  obtain ⟨_, rfl⟩ := (extractDependencies_iff program graph).mp extracted
  exact (checkProgram_iff program stage).mp accepted

end Spindle.Aggregation
