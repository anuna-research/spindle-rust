import Spindle.Aggregation.LoweringCorrectness

/-! Domain-free predicate aggregation and direct output binding.
This proves the aggregate operation, not the Rust potential-instance grounder. -/
namespace Spindle.Aggregation

def evalPredicateFold (σ : Arith.Substitution) (rows : List Pattern)
    (f : FoldExpression) : Except String (Option Int) := do
  let reducer ← reducerNamed f.reducer
  let seed ← f.seed.mapM (evalExpr σ)
  let values ← (rows.dedup.filterMap (matchRow σ f.pattern)).mapM
    (fun env => evalExpr env f.extract)
  return reducer.eval seed values

private theorem canonical (σ : Arith.Substitution) (rows : List Pattern)
    (f : FoldExpression) (result : Option Int) :
    evalPredicateFold σ rows f = .ok result ↔
      ∃ r seed values, Source.Names f.reducer r ∧ Source.Seed σ f.seed seed ∧
        List.Forall₂ (fun env n => evalExpr env f.extract = .ok n)
          (rows.dedup.filterMap (matchRow σ f.pattern)) values ∧
        Source.Reduces r seed values result := by
  unfold evalPredicateFold
  simp only [← names_iff, ← seed_iff, ← mapM_ok_iff, reduces_iff]
  cases hr : reducerNamed f.reducer with
  | error msg => simp [bind, Except.bind]
  | ok r =>
    cases hs : f.seed.mapM (evalExpr σ) with
    | error msg => simp [bind, Except.bind]
    | ok seed =>
      cases hv : (rows.dedup.filterMap (matchRow σ f.pattern)).mapM
          (fun env => evalExpr env f.extract) with
      | error msg => simp [bind, Except.bind]
      | ok values => simp [bind, Except.bind, pure, Except.pure]

/-- Soundness and completeness against the independent unordered row semantics. -/
theorem evalPredicateFold_iff (σ : Arith.Substitution) (rows : List Pattern)
    (f : FoldExpression) (result : Option Int) :
    evalPredicateFold σ rows f = .ok result ↔ Source.PredicateFold σ rows f result := by
  rw [canonical]
  constructor
  · rintro ⟨r, seed, values, name, initial, extracts, reduced⟩
    exact ⟨r, seed, values, name, initial,
      ⟨rows.dedup, List.nodup_dedup rows, fun _ => List.mem_dedup, extracts⟩, reduced⟩
  · rintro ⟨r, seed, values, name, initial, ⟨relation, unique, covers, extracts⟩, reduced⟩
    have rowsPerm : rows.dedup.Perm relation :=
      (List.perm_ext_iff_of_nodup (List.nodup_dedup rows) unique).mpr
        (fun row => List.mem_dedup.trans (covers row).symm)
    obtain ⟨canonical, hc, perm⟩ := List.perm_comp_forall₂
      (rowsPerm.filterMap (matchRow σ f.pattern)) extracts
    refine ⟨r, seed, canonical, name, initial, hc, ?_⟩
    apply (reduces_iff r seed result canonical).mpr
    rw [r.eval_permutation seed perm]
    exact (reduces_iff r seed result values).mp reduced

/-- The finite reference model agrees whenever its domain contains the result. -/
theorem predicateFold_finite_iff (domain : Arith.Domain) (σ : Arith.Substitution)
    (rows : List Pattern) (f : FoldExpression) (result : Option Int) :
    Source.Fold domain σ rows f result ↔
      Source.PredicateFold σ rows f result ∧
      (∀ n, result = some n → Arith.Term.integer n ∈ domain) := by
  constructor
  · rintro ⟨r, seed, values, name, initial, extracts, reduced, bounded⟩
    exact ⟨⟨r, seed, values, name, initial, extracts, reduced⟩, bounded⟩
  · rintro ⟨⟨r, seed, values, name, initial, extracts, reduced⟩, bounded⟩
    exact ⟨r, seed, values, name, initial, extracts, reduced, bounded⟩

/-- Existing output bindings are equality constraints; fresh outputs are inserted. -/
def bindPredicateResult (σ : Arith.Substitution) (v : String) (n : Int) :
    Option Arith.Substitution :=
  match σ.lookup v with
  | none => some (σ.set v (.integer n))
  | some t => if t = .integer n then some σ else none

theorem bindPredicateResult_fresh (σ : Arith.Substitution) (v : String) (n : Int)
    (fresh : σ.lookup v = none) :
    bindPredicateResult σ v n = some (σ.set v (.integer n)) := by
  simp [bindPredicateResult, fresh]

theorem bindPredicateResult_existing (σ : Arith.Substitution) (v : String)
    (n : Int) (t : Arith.Term) (existing : σ.lookup v = some t) :
    bindPredicateResult σ v n = some σ ↔ t = .integer n := by
  simp [bindPredicateResult, existing]

end Spindle.Aggregation
