import Spindle.Aggregation.Primitives
import Spindle.Arith.GroundRule


/-!
# Source aggregate semantics

This module does not import the lowerer. Expression evaluation and matching are
shared language primitives. Aggregate satisfaction is a relational judgment over
contributions, with an unordered reduction derivation. Whole rows have set
semantics; distinct rows with equal extracted integers still contribute twice.
Ordinary logical conditions are residual premises, not compile-time evidence.
-/
namespace Spindle.Aggregation.Source

/-- Finite choices for the outer variable scope. Local fold variables are bound
separately by matching each row. -/
inductive Assignment (domain : Arith.Domain) : List String → Arith.Substitution → Prop
  | empty : Assignment domain [] Arith.Substitution.empty
  | assign {vars σ} (v t) : t ∈ domain → Assignment domain vars σ →
      Assignment domain (v :: vars) (σ.set v t)

/-- The source language's supported aggregate operations. -/
inductive Names : String → Reducer Int → Prop
  | add : Names "+" sum
  | sum : Names "sum" Aggregation.sum
  | min : Names "min" minimum
  | max : Names "max" maximum

/-- An absent seed requests a nonempty aggregate; a present seed contributes once. -/
inductive Seed (σ : Arith.Substitution) : Option Expression → Option Int → Prop
  | absent : Seed σ none none
  | present {e n} : evalExpr σ e = .ok n → Seed σ (some e) (some n)

/-- Consume a bag of contributions in any order. No executable fold is used. -/
inductive Reduces (r : Reducer Int) : Option Int → List Int → Option Int → Prop
  | empty (seed) : Reduces r seed [] seed
  | first {xs rest x result} : xs.Perm (x :: rest) →
      Reduces r (some x) rest result → Reduces r none xs result
  | next {xs rest x seed result} : xs.Perm (x :: rest) →
      Reduces r (some (r.combine seed x)) rest result →
      Reduces r (some seed) xs result

/-- Every matching row supplies one extraction; extraction failures have no
successful judgment. Any duplicate-free enumeration of the input relation is
allowed. `Forall₂` preserves multiplicity of equal contributions. -/
def Contributions (σ : Arith.Substitution) (rows : List Pattern) (f : FoldExpression)
    (values : List Int) : Prop :=
  ∃ relation : List Pattern, relation.Nodup ∧ (∀ row, row ∈ relation ↔ row ∈ rows) ∧
    List.Forall₂ (fun env n => evalExpr env f.extract = .ok n)
      (relation.filterMap (matchRow σ f.pattern)) values

/-- Predicate aggregate semantics: computed results have no predeclared domain.
The relation and unordered reduction determine the value independently. -/
def PredicateFold (σ : Arith.Substitution) (rows : List Pattern)
    (f : FoldExpression) (result : Option Int) : Prop :=
  ∃ r seed values, Names f.reducer r ∧ Seed σ f.seed seed ∧
    Contributions σ rows f values ∧ Reduces r seed values result

/-- Finite-domain source satisfaction, including the empty required outcome. -/
def Fold (domain : Arith.Domain) (σ : Arith.Substitution) (rows : List Pattern)
    (f : FoldExpression) (result : Option Int) : Prop :=
  ∃ r seed values, Names f.reducer r ∧ Seed σ f.seed seed ∧
    Contributions σ rows f values ∧ Reduces r seed values result ∧
    (∀ n, result = some n → Arith.Term.integer n ∈ domain)

/-- Source folds depend only on membership of whole rows, never their order or
number of proof paths. Contributions from different rows retain multiplicity. -/
theorem Fold.rows_extensional (domain : Arith.Domain) (σ : Arith.Substitution)
    (xs ys : List Pattern) (f : FoldExpression) (result : Option Int)
    (same : ∀ row, row ∈ xs ↔ row ∈ ys) :
    Fold domain σ xs f result ↔ Fold domain σ ys f result := by
  constructor
  · rintro ⟨r, seed, values, name, initial, ⟨relation, unique, covers, extracts⟩, reduced, bounded⟩
    exact ⟨r, seed, values, name, initial,
      ⟨relation, unique, fun row => (covers row).trans (same row), extracts⟩, reduced, bounded⟩
  · rintro ⟨r, seed, values, name, initial, ⟨relation, unique, covers, extracts⟩, reduced, bounded⟩
    exact ⟨r, seed, values, name, initial,
      ⟨relation, unique, fun row => (covers row).trans (same row).symm, extracts⟩, reduced, bounded⟩

/-- Static conditions may be false without being undefined. Logic is residual. -/
inductive ConditionValue (domain : Arith.Domain) (σ : Arith.Substitution)
    (rows : List Pattern) : Condition → Bool → Prop
  | logic (p) : ConditionValue domain σ rows (.logic p) true
  | bind {v e n} : evalExpr σ e = .ok n →
      ConditionValue domain σ rows (.bind v e) (decide (σ.lookup v = some (.integer n)))
  | compare {op a b x y} : evalExpr σ a = .ok x → evalExpr σ b = .ok y →
      ConditionValue domain σ rows (.compare op a b) (compareInts op x y)
  | emptyFold {f} : Fold domain σ rows f none → ConditionValue domain σ rows (.fold f) false
  | fold {f n} : Fold domain σ rows f (some n) →
      ConditionValue domain σ rows (.fold f) (decide (σ.lookup f.resultVar = some (.integer n)))

def Enabled (domain : Arith.Domain) (σ : Arith.Substitution) (rows : List Pattern)
    (r : SchemaRule) : Prop := ∀ c ∈ r.body, ConditionValue domain σ rows c true

/-- Source clauses retain ordinary premises, all heads, and the original kind.
The closure requirement is recorded structurally, before encoding literals. -/
structure Clause where
  label : String
  kind : RuleType
  body : List Pattern
  head : Pattern
  closure : Bool
  deriving DecidableEq

def HasAggregate (r : SchemaRule) : Prop := ∃ f, Condition.fold f ∈ r.body

def InstanceClause (domain : Arith.Domain) (σ : Arith.Substitution) (rows : List Pattern)
    (r : SchemaRule) (c : Clause) : Prop :=
  Enabled domain σ rows r ∧ ∃ head ∈ r.heads,
    c.label = r.label ∧ c.kind = r.kind ∧ c.head = head.applySubst σ ∧
    c.body = (r.premises.map (Pattern.applySubst σ)) ∧
    (c.closure = true ↔ HasAggregate r)

end Spindle.Aggregation.Source
