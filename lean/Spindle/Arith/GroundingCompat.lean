/-
  Spindle Arithmetic–Grounding Compatibility

  Proves that arithmetic constraint evaluation and literal grounding are
  compatible when driven by the same substitution (SPEC-017).

  The bridge: a single Substitution induces both
    (a) a literal grounding (via applyTerm/applySubst), and
    (b) a ValueEnv (via toValueEnv) for evaluating arithmetic constraints.

  Main results:
    • toValueEnv properties: hit/miss/groundFor
    • ground_preserves_constraints: grounding does not change constraint evaluation
    • groundWith_success: complete substitution + satisfied constraints → ground ArithRule
    • groundWith_isGround / groundWith_constraints_satisfied: extraction from success
-/
import Spindle.Arith.Constraint
import Spindle.Arith.GroundRule

namespace Spindle.Arith

/-! ## Bridge: Substitution → ValueEnv -/

/-- A variable name table maps arithmetic VarIds to substitution variable names.
    This bridges the two variable namespaces (Nat vs String). -/
abbrev VarNameTable := VarId → Option String

/-- Convert a term-level substitution to a value environment for arithmetic
    evaluation, given a mapping from VarIds to variable names. -/
def Substitution.toValueEnv (σ : Substitution) (names : VarNameTable) : ValueEnv :=
  fun id =>
    match names id with
    | none => none
    | some name =>
      match σ.lookup name with
      | none => none
      | some t => t.toValue

/-! ## Properties of toValueEnv -/

/-- If the name table maps `id` to `name`, and the substitution maps `name` to
    a numeric term `t`, then the induced ValueEnv returns `t.toValue`. -/
theorem Substitution.toValueEnv_hit (σ : Substitution) (names : VarNameTable)
    (id : VarId) (name : String) (t : Term) (v : Value)
    (hname : names id = some name)
    (hlook : σ.lookup name = some t)
    (hval : t.toValue = some v) :
    σ.toValueEnv names id = some v := by
  simp [Substitution.toValueEnv, hname, hlook, hval]

/-- If the name table does not map `id`, the induced ValueEnv returns `none`. -/
theorem Substitution.toValueEnv_miss_name (σ : Substitution) (names : VarNameTable)
    (id : VarId) (hname : names id = none) :
    σ.toValueEnv names id = none := by
  simp [Substitution.toValueEnv, hname]

/-- If the substitution does not bind the name, the induced ValueEnv returns `none`. -/
theorem Substitution.toValueEnv_miss_subst (σ : Substitution) (names : VarNameTable)
    (id : VarId) (name : String) (hname : names id = some name)
    (hlook : σ.lookup name = none) :
    σ.toValueEnv names id = none := by
  simp [Substitution.toValueEnv, hname, hlook]

/-! ## Groundness of induced ValueEnv -/

/-- A substitution resolves a VarId under a name table if the induced ValueEnv
    returns a value for it. -/
def Substitution.resolves (σ : Substitution) (names : VarNameTable) (id : VarId) : Prop :=
  σ.toValueEnv names id ≠ none

/-- Resolution is equivalent to groundFor for the singleton variable expression. -/
theorem Substitution.resolves_iff_groundFor_var (σ : Substitution) (names : VarNameTable)
    (id : VarId) :
    σ.resolves names id ↔ ValueEnv.groundFor (σ.toValueEnv names) (.var id) := by
  simp [Substitution.resolves, ValueEnv.groundFor, ArithExpr.varIds]

/-- A variable name table covers an expression if every VarId in the expression
    is resolved by the substitution. -/
def VarNameTable.coversExpr (names : VarNameTable) (σ : Substitution) (e : ArithExpr) : Prop :=
  ∀ id, id ∈ e.varIds → σ.resolves names id

/-- Covering an expression is equivalent to ValueEnv.groundFor. -/
theorem VarNameTable.coversExpr_iff_groundFor (names : VarNameTable) (σ : Substitution)
    (e : ArithExpr) :
    names.coversExpr σ e ↔ ValueEnv.groundFor (σ.toValueEnv names) e := by
  simp [VarNameTable.coversExpr, ValueEnv.groundFor, Substitution.resolves]

/-- A variable name table covers a constraint if it covers all variables in it. -/
def VarNameTable.coversConstraint (names : VarNameTable) (σ : Substitution) (c : ArithConstraint) : Prop :=
  match c with
  | .bind _ expr => names.coversExpr σ expr
  | .compare _ lhs rhs => names.coversExpr σ lhs ∧ names.coversExpr σ rhs

/-- A variable name table covers a list of constraints. -/
def VarNameTable.coversConstraints (names : VarNameTable) (σ : Substitution) (cs : List ArithConstraint) : Prop :=
  ∀ c, c ∈ cs → names.coversConstraint σ c

/-! ## Numeric substitution -/

/-- A substitution is numeric if every term in its range has a Value representation. -/
def Substitution.isNumeric (σ : Substitution) : Bool :=
  σ.map.all fun (_, t) => t.toValue.isSome

/-- Helper: if lookup succeeds in an association list, the result pair is in the list. -/
private theorem lookup_mem_of_some {pairs : List (String × Term)} {v : String} {t : Term}
    (h : pairs.lookup v = some t) : (v, t) ∈ pairs := by
  induction pairs with
  | nil => simp [List.lookup] at h
  | cons p ps ih =>
    simp only [List.lookup] at h
    split at h
    · next heq =>
      have hv := beq_iff_eq.mp heq
      have ht := Option.some.inj h
      refine List.mem_cons.mpr (Or.inl ?_)
      cases p with
      | mk k w => exact Prod.ext hv ht.symm
    · exact List.mem_cons.mpr (Or.inr (ih h))

/-- Helper: if all pairs satisfy a predicate and a pair is in the list,
    the pair satisfies the predicate. -/
private theorem all_mem_pred {pairs : List (String × Term)} {v : String} {t : Term}
    (hall : pairs.all (fun p => p.2.toValue.isSome) = true)
    (hmem : (v, t) ∈ pairs) :
    t.toValue.isSome = true := by
  induction pairs with
  | nil => simp at hmem
  | cons p ps ih =>
    simp only [List.all_cons, Bool.and_eq_true] at hall
    cases List.mem_cons.mp hmem with
    | inl heq =>
      have := congrArg Prod.snd heq
      simp at this
      rw [this]
      exact hall.1
    | inr hmem' => exact ih hall.2 hmem'

/-- A numeric substitution that maps a name to a term guarantees the term has a value. -/
theorem Substitution.lookup_toValue_of_numeric (σ : Substitution) (name : String) (t : Term)
    (hnum : σ.isNumeric = true) (hlook : σ.lookup name = some t) :
    t.toValue.isSome = true := by
  exact all_mem_pred hnum (lookup_mem_of_some hlook)

/-- A numeric substitution resolves any VarId whose name is in the domain. -/
theorem Substitution.resolves_of_numeric (σ : Substitution) (names : VarNameTable)
    (id : VarId) (name : String)
    (hnum : σ.isNumeric = true)
    (hname : names id = some name)
    (hdom : σ.inDomain name = true) :
    σ.resolves names id := by
  simp [Substitution.resolves, Substitution.toValueEnv, hname]
  simp [Substitution.inDomain] at hdom
  obtain ⟨t, hlook⟩ := Option.isSome_iff_exists.mp hdom
  rw [hlook]
  have hsome := σ.lookup_toValue_of_numeric name t hnum hlook
  match ht : t.toValue with
  | some v => simp [ht]
  | none => simp [ht] at hsome

/-- A numeric substitution that covers all constraint variables produces
    a ground ValueEnv for those variables. -/
theorem Substitution.toValueEnv_groundFor_numeric (σ : Substitution) (names : VarNameTable)
    (e : ArithExpr) (hnum : σ.isNumeric = true)
    (hcov : ∀ id, id ∈ e.varIds → ∃ name, names id = some name ∧ σ.inDomain name = true) :
    ValueEnv.groundFor (σ.toValueEnv names) e := by
  rw [← VarNameTable.coversExpr_iff_groundFor]
  intro id hid
  obtain ⟨name, hname, hdom⟩ := hcov id hid
  exact σ.resolves_of_numeric names id name hnum hname hdom

/-! ## ArithRule: rule with arithmetic constraints -/

/-- A rule extended with arithmetic constraints in its body.
    Models a Spindle rule where the body contains both literals (for matching)
    and arithmetic constraints (for numeric guards). -/
structure ArithRule where
  /-- The underlying rule (head + literal body). -/
  rule : Rule
  /-- Arithmetic constraints that must be satisfied. -/
  constraints : List ArithConstraint
  deriving Repr, Inhabited

/-- An ArithRule is ground if its underlying rule is ground. -/
def ArithRule.isGround (ar : ArithRule) : Bool :=
  ar.rule.isGround

/-- Apply a substitution to an ArithRule (only affects the literal part).
    Arithmetic constraints are not textually substituted — they are evaluated
    against the ValueEnv induced by the substitution. -/
def ArithRule.applySubst (σ : Substitution) (ar : ArithRule) : ArithRule :=
  { rule := ar.rule.applySubst σ
    constraints := ar.constraints }

/-! ## Constraint satisfaction result for ArithRules -/

/-- Result of checking an ArithRule under a substitution: the rule is ground
    and all constraints are satisfied. -/
structure ArithGroundingResult where
  /-- The ground rule produced by applying the substitution. -/
  groundRule : Rule
  /-- The final value environment after constraint evaluation. -/
  finalEnv : ValueEnv

/-- Check whether a substitution grounds an ArithRule and satisfies all constraints. -/
def ArithRule.groundWith (ar : ArithRule) (σ : Substitution) (names : VarNameTable) :
    Option ArithGroundingResult :=
  let groundR := ar.rule.applySubst σ
  if groundR.isGround then
    let env := σ.toValueEnv names
    match evalConstraints env ar.constraints with
    | .satisfied env' => some ⟨groundR, env'⟩
    | _ => none
  else none

/-! ## Main compatibility theorems -/

/-- **Theorem 1**: Applying a complete substitution to an ArithRule produces a ground rule.
    This lifts Rule.applySubst_ground to ArithRules. -/
theorem ArithRule.applySubst_ground (σ : Substitution) (ar : ArithRule)
    (hcomplete : σ.completeRule ar.rule = true) :
    (ar.applySubst σ).isGround = true := by
  simp [ArithRule.isGround, ArithRule.applySubst]
  exact Rule.applySubst_ground σ ar.rule hcomplete

/-- **Theorem 2**: Grounding preserves constraint satisfaction.
    If constraints are satisfied under the substitution-induced ValueEnv,
    then the ground ArithRule's constraints are also satisfied (trivially,
    since grounding does not modify constraints — it only grounds literals). -/
theorem ArithRule.ground_preserves_constraints (σ : Substitution) (ar : ArithRule)
    (names : VarNameTable) (env' : ValueEnv)
    (hsat : evalConstraints (σ.toValueEnv names) ar.constraints = .satisfied env') :
    evalConstraints (σ.toValueEnv names) (ar.applySubst σ).constraints = .satisfied env' := by
  simp [ArithRule.applySubst]
  exact hsat

/-- **Theorem 3 (Main)**: A complete substitution that satisfies all constraints
    successfully grounds an ArithRule.
    This is the core compatibility result: a single substitution can consistently
    ground both the literal body and satisfy the arithmetic constraints. -/
theorem ArithRule.groundWith_success (ar : ArithRule) (σ : Substitution)
    (names : VarNameTable) (env' : ValueEnv)
    (hcomplete : σ.completeRule ar.rule = true)
    (hsat : evalConstraints (σ.toValueEnv names) ar.constraints = .satisfied env') :
    ar.groundWith σ names = some ⟨ar.rule.applySubst σ, env'⟩ := by
  simp [ArithRule.groundWith]
  have hgnd := Rule.applySubst_ground σ ar.rule hcomplete
  simp [hgnd, hsat]

/-- **Theorem 4**: If groundWith succeeds, the resulting rule is ground. -/
theorem ArithRule.groundWith_isGround (ar : ArithRule) (σ : Substitution)
    (names : VarNameTable) (result : ArithGroundingResult)
    (hsuccess : ar.groundWith σ names = some result) :
    result.groundRule.isGround = true := by
  simp only [ArithRule.groundWith] at hsuccess
  by_cases hgnd : (ar.rule.applySubst σ).isGround = true
  · simp [hgnd] at hsuccess
    match heval : evalConstraints (σ.toValueEnv names) ar.constraints with
    | .satisfied env' =>
      rw [heval] at hsuccess
      have heq : result = ⟨ar.rule.applySubst σ, env'⟩ := by
        simp at hsuccess; exact hsuccess.symm
      rw [heq]
      exact hgnd
    | .unsatisfied => rw [heval] at hsuccess; simp at hsuccess
    | .error e => rw [heval] at hsuccess; simp at hsuccess
  · simp [hgnd] at hsuccess

/-- **Theorem 5**: If groundWith succeeds, the constraints were satisfied. -/
theorem ArithRule.groundWith_constraints_satisfied (ar : ArithRule) (σ : Substitution)
    (names : VarNameTable) (result : ArithGroundingResult)
    (hsuccess : ar.groundWith σ names = some result) :
    evalConstraints (σ.toValueEnv names) ar.constraints = .satisfied result.finalEnv := by
  simp only [ArithRule.groundWith] at hsuccess
  by_cases hgnd : (ar.rule.applySubst σ).isGround = true
  · simp [hgnd] at hsuccess
    match heval : evalConstraints (σ.toValueEnv names) ar.constraints with
    | .satisfied env' =>
      rw [heval] at hsuccess
      have heq : result = ⟨ar.rule.applySubst σ, env'⟩ := by
        simp at hsuccess; exact hsuccess.symm
      simp [heq]
    | .unsatisfied => rw [heval] at hsuccess; simp at hsuccess
    | .error e => rw [heval] at hsuccess; simp at hsuccess
  · simp [hgnd] at hsuccess

/-! ## Corollaries -/

/-- **Corollary**: A ground covering substitution that satisfies constraints
    successfully grounds an ArithRule. Combines the covering corollary from
    GroundRule with constraint satisfaction. -/
theorem ArithRule.groundWith_of_covering (ar : ArithRule) (σ : Substitution)
    (names : VarNameTable) (env' : ValueEnv)
    (hgnd : σ.isGround = true)
    (hcov : ∀ v, v ∈ ar.rule.vars → σ.inDomain v = true)
    (hsat : evalConstraints (σ.toValueEnv names) ar.constraints = .satisfied env') :
    ar.groundWith σ names = some ⟨ar.rule.applySubst σ, env'⟩ := by
  have hcomplete := Substitution.complete_of_ground_covering_rule σ ar.rule hgnd hcov
  exact ArithRule.groundWith_success ar σ names env' hcomplete hsat

/-- The empty constraint list is always satisfied under any substitution. -/
theorem ArithRule.groundWith_no_constraints (ar : ArithRule) (σ : Substitution)
    (names : VarNameTable)
    (hno : ar.constraints = [])
    (hcomplete : σ.completeRule ar.rule = true) :
    ar.groundWith σ names = some ⟨ar.rule.applySubst σ, σ.toValueEnv names⟩ := by
  simp [ArithRule.groundWith]
  have hgnd := Rule.applySubst_ground σ ar.rule hcomplete
  simp [hgnd, hno, evalConstraints]

/-- If a substitution produces the same groundWith result twice, the results are equal. -/
theorem ArithRule.groundWith_result_eq (ar : ArithRule) (σ : Substitution)
    (names : VarNameTable) (r₁ r₂ : ArithGroundingResult)
    (h₁ : ar.groundWith σ names = some r₁)
    (h₂ : ar.groundWith σ names = some r₂) :
    r₁ = r₂ := by
  rw [h₁] at h₂
  exact Option.some.inj h₂

end Spindle.Arith
