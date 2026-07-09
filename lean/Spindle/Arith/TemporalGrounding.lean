/-
  Spindle Temporal Grounding Constraints

  Formalizes AllenConstraint evaluation during grounding phase 2 (SPEC-020).
  An AllenConstraint binds two interval variable names and an Allen relation;
  during grounding both variables must be resolved to concrete Interval values
  and the relation must hold.

  Proves:
  1. A satisfied result is witnessed by the relation classification matching.
  2. For proper intervals, satisfaction implies AllenRelation.holds.
  3. If a constraint list is satisfied, every individual constraint is satisfied.
  4. Binding a variable absent from a constraint preserves its evaluation.
-/
import Spindle.Arith.AllenRelation
import Spindle.Arith.Substitution

namespace Spindle.Arith

/-! ## Interval binding map -/

/-- A finite map from interval variable names to concrete intervals.
    During grounding phase 2, semi-naive evaluation accumulates these
    bindings as body atoms are matched against the current interpretation. -/
abbrev IntervalEnv := List (String × Interval)

/-- Look up an interval variable in the environment. -/
def IntervalEnv.find (env : IntervalEnv) (v : String) : Option Interval :=
  (env.lookup v : Option Interval)

/-- The empty interval environment. -/
def IntervalEnv.empty : IntervalEnv := []

/-- Extend the environment with a new interval binding.
    If the variable is already bound the existing binding wins (no override). -/
def IntervalEnv.extend (env : IntervalEnv) (v : String) (i : Interval) : IntervalEnv :=
  match env.find v with
  | some _ => env
  | none   => (v, i) :: env

/-- extend makes the new variable available when it was previously unbound. -/
theorem IntervalEnv.find_extend_new (env : IntervalEnv) (v : String) (i : Interval)
    (h : env.find v = none) :
    (env.extend v i).find v = some i := by
  simp only [IntervalEnv.extend, h]
  simp [IntervalEnv.find, List.lookup]

/-- extend leaves the existing binding intact when the variable is already bound. -/
theorem IntervalEnv.find_extend_existing (env : IntervalEnv) (v : String) (i j : Interval)
    (h : env.find v = some i) :
    (env.extend v j).find v = some i := by
  simp only [IntervalEnv.extend, h]

/-- extend does not affect bindings of other variables. -/
theorem IntervalEnv.find_extend_other (env : IntervalEnv) (v w : String) (i : Interval)
    (hne : v ≠ w) :
    (env.extend v i).find w = env.find w := by
  simp only [IntervalEnv.extend]
  split
  · rfl
  · simp [IntervalEnv.find, List.lookup, BEq.beq, hne.symm]

/-! ## AllenConstraint type -/

/-- An Allen temporal constraint as it appears in a rule body.
    Constrains that the Allen relation `rel` must hold between the
    intervals bound to `lhsVar` and `rhsVar` when both are ground. -/
structure AllenConstraint where
  /-- Name of the left-hand interval variable. -/
  lhsVar : String
  /-- Name of the right-hand interval variable. -/
  rhsVar : String
  /-- The required Allen relation between lhs and rhs. -/
  rel    : AllenRelation
  deriving Repr, BEq, DecidableEq

/-! ## Result type -/

/-- Result of evaluating an AllenConstraint. -/
inductive AllenResult where
  /-- Both variables are bound and the relation holds. -/
  | satisfied
  /-- Both variables are bound but the relation does not hold. -/
  | unsatisfied
  /-- At least one variable is not yet bound (constraint deferred). -/
  | unbound
  deriving Repr, BEq, DecidableEq, Inhabited

/-! ## Decidable check via classify -/

/-- Decision procedure for whether an Allen relation holds between two intervals.
    Uses `classify` (total and computable) via `DecidableEq` on `AllenRelation`. -/
def AllenRelation.check (r : AllenRelation) (x y : Interval) : Bool :=
  if r = AllenRelation.classify x y then true else false

/-- `check` returns `true` iff the relation equals `classify x y`. -/
theorem AllenRelation.check_eq_classify (r : AllenRelation) (x y : Interval) :
    r.check x y = true ↔ r = AllenRelation.classify x y := by
  simp [check]

/-- For proper intervals: `check` iff `holds`. -/
theorem AllenRelation.check_iff_holds {r : AllenRelation} {x y : Interval}
    (hx : x.proper) (hy : y.proper) :
    r.check x y = true ↔ r.holds x y := by
  rw [check_eq_classify]
  constructor
  · intro h; rw [h]; exact classify_holds x y
  · intro h; exact holds_unique hx hy h

/-- `check` returns `false` iff the relation does NOT equal `classify x y`. -/
theorem AllenRelation.check_false_iff (r : AllenRelation) (x y : Interval) :
    r.check x y = false ↔ r ≠ AllenRelation.classify x y := by
  simp [check]

/-! ## Evaluation -/

/-- Evaluate an AllenConstraint against an IntervalEnv.
    - Returns `satisfied` if both variables are bound and the relation holds.
    - Returns `unsatisfied` if both variables are bound but the relation fails.
    - Returns `unbound` if at least one variable is not yet bound. -/
def evaluateConstraint (env : IntervalEnv) (c : AllenConstraint) : AllenResult :=
  match env.find c.lhsVar, env.find c.rhsVar with
  | some lhs, some rhs =>
    if c.rel.check lhs rhs then .satisfied else .unsatisfied
  | _, _ => .unbound

/-! ## Witness theorems -/

/-- **Satisfaction witness**: `satisfied` implies both variables are bound and
    `check` returns `true`. -/
theorem evaluateConstraint_satisfied_witness
    (env : IntervalEnv) (c : AllenConstraint)
    (h : evaluateConstraint env c = .satisfied) :
    ∃ lhs rhs,
      env.find c.lhsVar = some lhs ∧
      env.find c.rhsVar = some rhs ∧
      c.rel.check lhs rhs = true := by
  simp only [evaluateConstraint] at h
  split at h
  · next lhs rhs hlhs hrhs =>
    split at h
    · next hcheck => exact ⟨lhs, rhs, hlhs, hrhs, hcheck⟩
    · exact absurd h (by simp)
  · exact absurd h (by simp)

/-- **Proper satisfaction implies holds**: for proper intervals, `satisfied`
    means the Allen relation genuinely holds. -/
theorem evaluateConstraint_holds_of_satisfied
    (env : IntervalEnv) (c : AllenConstraint)
    (lhs rhs : Interval)
    (hlhs : env.find c.lhsVar = some lhs)
    (hrhs : env.find c.rhsVar = some rhs)
    (hx : lhs.proper) (hy : rhs.proper)
    (hsat : evaluateConstraint env c = .satisfied) :
    c.rel.holds lhs rhs := by
  obtain ⟨lhs', rhs', hlhs', hrhs', hcheck⟩ :=
    evaluateConstraint_satisfied_witness env c hsat
  have heqlhs : lhs = lhs' := by rw [hlhs] at hlhs'; exact Option.some.inj hlhs'
  have heqrhs : rhs = rhs' := by rw [hrhs] at hrhs'; exact Option.some.inj hrhs'
  subst heqlhs; subst heqrhs
  exact (AllenRelation.check_iff_holds hx hy).mp hcheck

/-- **Unsatisfied witness**: `unsatisfied` implies both variables are bound and
    the check returns `false`. -/
theorem evaluateConstraint_unsatisfied_witness
    (env : IntervalEnv) (c : AllenConstraint)
    (h : evaluateConstraint env c = .unsatisfied) :
    ∃ lhs rhs,
      env.find c.lhsVar = some lhs ∧
      env.find c.rhsVar = some rhs ∧
      c.rel.check lhs rhs = false := by
  simp only [evaluateConstraint] at h
  split at h
  · next lhs rhs hlhs hrhs =>
    split at h
    · exact absurd h (by simp)
    · next hcheck =>
      exact ⟨lhs, rhs, hlhs, hrhs, Bool.eq_false_iff.mpr (by simpa using hcheck)⟩
  · exact absurd h (by simp)

/-- **Unbound characterisation**: `unbound` iff at least one variable is missing. -/
theorem evaluateConstraint_unbound_iff
    (env : IntervalEnv) (c : AllenConstraint) :
    evaluateConstraint env c = .unbound ↔
      env.find c.lhsVar = none ∨ env.find c.rhsVar = none := by
  simp only [evaluateConstraint]
  constructor
  · intro h
    split at h
    · next _ _ _ _ => split at h <;> simp at h
    · next =>
      rcases Option.eq_none_or_eq_some (env.find c.lhsVar) with hlhs | ⟨lhs, hlhs⟩
      · exact Or.inl hlhs
      · rcases Option.eq_none_or_eq_some (env.find c.rhsVar) with hrhs | ⟨rhs, hrhs⟩
        · exact Or.inr hrhs
        · simp_all
  · intro h
    rcases h with hlhs | hrhs
    · simp [hlhs]
    · rcases Option.eq_none_or_eq_some (env.find c.lhsVar) with hlhs | ⟨lhs, hlhs⟩
      · simp [hlhs]
      · simp [hlhs, hrhs]

/-! ## Integration with grounding substitutions -/

/-- A joint grounding state pairs the logical substitution (for term variables)
    with an interval environment (for interval variables).
    This models how grounding phase 2 threads both kinds of bindings. -/
structure GroundingState where
  /-- Bindings for term-level variables (symbols, numbers). -/
  subst : Substitution
  /-- Bindings for interval-typed variables. -/
  intervals : IntervalEnv
  deriving Repr, Inhabited

/-- The empty grounding state (no variables bound). -/
def GroundingState.empty : GroundingState :=
  ⟨Substitution.empty, IntervalEnv.empty⟩

/-- Bind an interval variable in the grounding state. -/
def GroundingState.bindInterval (gs : GroundingState) (v : String) (i : Interval) :
    GroundingState :=
  { gs with intervals := gs.intervals.extend v i }

/-- Check an Allen constraint against a grounding state. -/
def GroundingState.checkConstraint (gs : GroundingState) (c : AllenConstraint) : AllenResult :=
  evaluateConstraint gs.intervals c

/-- Binding an interval variable that doesn't appear in a constraint
    preserves the constraint's evaluation result. -/
theorem GroundingState.checkConstraint_preserved_by_bind
    (gs : GroundingState) (c : AllenConstraint) (v : String) (i : Interval)
    (hv_lhs : v ≠ c.lhsVar) (hv_rhs : v ≠ c.rhsVar) :
    (gs.bindInterval v i).checkConstraint c = gs.checkConstraint c := by
  simp only [GroundingState.checkConstraint, GroundingState.bindInterval]
  simp only [evaluateConstraint]
  rw [IntervalEnv.find_extend_other _ _ _ _ hv_lhs,
      IntervalEnv.find_extend_other _ _ _ _ hv_rhs]

/-! ## Constraint list evaluation -/

/-- Evaluate a list of Allen constraints sequentially.
    Returns `satisfied` only if all constraints hold,
    `unsatisfied` at the first failure, `unbound` at the first unresolved variable. -/
def evaluateConstraints (env : IntervalEnv) : List AllenConstraint → AllenResult
  | []      => .satisfied
  | c :: cs =>
    match evaluateConstraint env c with
    | .satisfied   => evaluateConstraints env cs
    | .unsatisfied => .unsatisfied
    | .unbound     => .unbound

/-- Empty constraint list is always satisfied. -/
theorem evaluateConstraints_nil (env : IntervalEnv) :
    evaluateConstraints env [] = .satisfied := rfl

/-- If a constraint list evaluates to `satisfied`, every individual constraint
    is satisfied. -/
theorem evaluateConstraints_all_satisfied
    (env : IntervalEnv) (cs : List AllenConstraint)
    (h : evaluateConstraints env cs = .satisfied) :
    ∀ c ∈ cs, evaluateConstraint env c = .satisfied := by
  induction cs with
  | nil => simp
  | cons c rest ih =>
    intro c' hc'
    simp only [evaluateConstraints] at h
    split at h
    · next hc =>
      rcases List.mem_cons.mp hc' with rfl | hmem
      · exact hc
      · exact ih h c' hmem
    · simp at h
    · simp at h

end Spindle.Arith
