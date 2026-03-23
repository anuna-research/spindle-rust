/-
  Spindle Pattern Matching

  Defines pattern matching: given a non-ground literal (pattern) and a ground
  literal (target), compute the most general unifier (MGU) if one exists.
  Proves: if match succeeds, applying the substitution to the pattern yields
  the target (SPEC-017).
-/
import Spindle.Arith.GroundLiteral

namespace Spindle.Arith

/-! ## Term BEq reflection -/

/-- BEq true implies propositional equality for Term. -/
theorem Term.eq_of_beq {a b : Term} (h : (a == b) = true) : a = b := by
  cases a <;> cases b <;> simp [BEq.beq] at h <;> try (obtain rfl := h; rfl)
  · obtain ⟨h1, h2⟩ := h; subst h1; subst h2; rfl

/-! ## Term-level matching -/

/-- Match a single pattern term against a ground target term.
    Returns an updated substitution on success, or `none` on failure.
    - Variable: bind to target if unbound, check consistency if already bound
    - Ground term: must equal target exactly -/
def matchTerm (σ : Substitution) (pat target : Term) : Option Substitution :=
  match pat with
  | .variable v =>
    match σ.lookup v with
    | some t => if t == target then some σ else none
    | none => some (⟨(v, target) :: σ.map⟩)
  | t => if t == target then some σ else none

/-- Match two lists of terms pairwise, threading the substitution. -/
def matchTerms (σ : Substitution) : List Term → List Term → Option Substitution
  | [], [] => some σ
  | p :: ps, t :: ts => match matchTerm σ p t with
    | some σ' => matchTerms σ' ps ts
    | none => none
  | _, _ => none

/-! ## Literal-level matching -/

/-- Match a pattern literal against a ground target literal.
    Returns the MGU substitution on success. Requires:
    - Same predicate name
    - Same negation flag
    - Same arity (argument count)
    - Each argument pair matches -/
def matchLiteral (pat target : Literal) : Option Substitution :=
  if pat.name != target.name then none
  else if pat.negation != target.negation then none
  else matchTerms Substitution.empty pat.args target.args

/-! ## Helper: if-then-some-else-none elimination -/

private theorem if_some_eq_some {c : Bool} {a b : α} (h : (if c then some a else none) = some b) :
    c = true ∧ a = b := by
  cases c <;> simp at h
  exact ⟨rfl, h⟩

/-! ## Soundness: matchTerm preserves earlier bindings -/

/-- matchTerm only extends σ; it never changes existing bindings. -/
theorem matchTerm_extends (σ : Substitution) (pat target : Term) (σ' : Substitution)
    (hmatch : matchTerm σ pat target = some σ')
    (v : String) (t : Term) (hlook : σ.lookup v = some t) :
    σ'.lookup v = some t := by
  unfold matchTerm at hmatch
  split at hmatch
  · -- pat = variable w
    rename_i w
    cases hlookw : σ.lookup w with
    | some tw =>
      simp only [hlookw] at hmatch
      have ⟨_, hσeq⟩ := if_some_eq_some hmatch
      subst hσeq; exact hlook
    | none =>
      simp only [hlookw] at hmatch
      have hσeq : σ' = ⟨(w, target) :: σ.map⟩ := by
        cases hmatch; rfl
      subst hσeq
      simp only [Substitution.lookup, List.lookup]
      have hvw : (v == w) = false := by
        cases hvw : (v == w)
        · rfl
        · have := eq_of_beq hvw; rw [← this] at hlookw; simp [hlook] at hlookw
      simp only [hvw]
      exact hlook
  · -- pat is ground
    have ⟨_, hσeq⟩ := if_some_eq_some hmatch
    subst hσeq; exact hlook

/-! ## Key lemma: matchTerm correctness -/

/-- If matchTerm succeeds, applying σ' to pat yields target. -/
theorem matchTerm_sound (σ : Substitution) (pat target : Term) (σ' : Substitution)
    (hmatch : matchTerm σ pat target = some σ') :
    σ'.applyTerm pat = target := by
  unfold matchTerm at hmatch
  split at hmatch
  · -- pat = variable v
    rename_i v
    cases hlook : σ.lookup v with
    | some t =>
      simp only [hlook] at hmatch
      have ⟨heq, hσeq⟩ := if_some_eq_some hmatch
      subst hσeq
      simp only [Substitution.applyTerm, hlook]
      exact Term.eq_of_beq heq
    | none =>
      simp only [hlook] at hmatch
      have hσeq : σ' = ⟨(v, target) :: σ.map⟩ := by cases hmatch; rfl
      subst hσeq
      simp [Substitution.applyTerm, Substitution.lookup, List.lookup]
  · -- pat is not a variable
    rename_i t _
    have ⟨heq, hσeq⟩ := if_some_eq_some hmatch
    subst hσeq
    have hteq := Term.eq_of_beq heq
    cases t <;> simp_all [Substitution.applyTerm]

/-! ## matchTerms extends σ -/

/-- matchTerms only extends σ; it never changes existing bindings. -/
theorem matchTerms_extends (σ : Substitution) (pats targets : List Term) (σ' : Substitution)
    (hmatch : matchTerms σ pats targets = some σ')
    (v : String) (t : Term) (hlook : σ.lookup v = some t) :
    σ'.lookup v = some t := by
  induction pats generalizing σ targets with
  | nil =>
    cases targets with
    | nil => simp [matchTerms] at hmatch; subst hmatch; exact hlook
    | cons _ _ => simp [matchTerms] at hmatch
  | cons p ps ih =>
    cases targets with
    | nil => simp [matchTerms] at hmatch
    | cons t' ts =>
      simp only [matchTerms] at hmatch
      cases hmid : matchTerm σ p t' with
      | none => simp [hmid] at hmatch
      | some σ_mid =>
        simp [hmid] at hmatch
        exact ih σ_mid ts hmatch (matchTerm_extends σ p t' σ_mid hmid v t hlook)

/-! ## matchTerms soundness -/

/-- Helper: matchTerm for a variable gives us the lookup in the result. -/
private theorem matchTerm_variable_lookup (σ : Substitution) (v : String) (target : Term)
    (σ' : Substitution) (hmatch : matchTerm σ (.variable v) target = some σ') :
    σ'.lookup v = some target := by
  simp only [matchTerm] at hmatch
  cases hlook : σ.lookup v with
  | some t =>
    simp only [hlook] at hmatch
    have ⟨heq, hσeq⟩ := if_some_eq_some hmatch
    subst hσeq
    rw [hlook]; congr 1; exact Term.eq_of_beq heq
  | none =>
    simp only [hlook] at hmatch
    have hσeq : σ' = ⟨(v, target) :: σ.map⟩ := by cases hmatch; rfl
    subst hσeq
    simp [Substitution.lookup, List.lookup]

/-- If matchTerms succeeds, applying σ' to each pattern yields the targets. -/
theorem matchTerms_sound (σ : Substitution) (pats targets : List Term) (σ' : Substitution)
    (hmatch : matchTerms σ pats targets = some σ') :
    σ'.applyTerms pats = targets := by
  induction pats generalizing σ targets with
  | nil =>
    cases targets with
    | nil => simp [Substitution.applyTerms]
    | cons _ _ => simp [matchTerms] at hmatch
  | cons p ps ih =>
    cases targets with
    | nil => simp [matchTerms] at hmatch
    | cons t ts =>
      simp only [matchTerms] at hmatch
      cases hmid : matchTerm σ p t with
      | none => simp [hmid] at hmatch
      | some σ_mid =>
        simp [hmid] at hmatch
        simp only [Substitution.applyTerms, List.map_cons, List.cons.injEq]
        refine ⟨?_, ih σ_mid ts hmatch⟩
        -- Need: σ'.applyTerm p = t
        cases p with
        | «variable» v =>
          simp only [Substitution.applyTerm]
          have hlook_mid := matchTerm_variable_lookup σ v t σ_mid hmid
          have hlook' := matchTerms_extends σ_mid ps ts σ' hmatch v t hlook_mid
          simp [hlook']
        | symbol s =>
          have := matchTerm_sound σ (.symbol s) t σ_mid hmid
          simp [Substitution.applyTerm] at this ⊢; exact this
        | integer n =>
          have := matchTerm_sound σ (.integer n) t σ_mid hmid
          simp [Substitution.applyTerm] at this ⊢; exact this
        | decimal n sc =>
          have := matchTerm_sound σ (.decimal n sc) t σ_mid hmid
          simp [Substitution.applyTerm] at this ⊢; exact this

/-! ## Main theorem: matchLiteral soundness -/

/-- **Main theorem**: if `matchLiteral pat target` succeeds with substitution `σ`,
    then applying `σ` to `pat` yields `target`.

    This formalizes the correctness of pattern matching: the computed substitution
    is a unifier of the pattern and target literals. -/
theorem matchLiteral_sound (pat target : Literal) (σ : Substitution)
    (hmatch : matchLiteral pat target = some σ) :
    pat.applySubst σ = target := by
  simp only [matchLiteral] at hmatch
  split at hmatch
  · simp at hmatch
  · split at hmatch
    · simp at hmatch
    · rename_i hname hneg
      simp [bne] at hname hneg
      have hargs := matchTerms_sound Substitution.empty pat.args target.args σ hmatch
      simp only [Literal.applySubst]
      cases pat with
      | mk pname pneg pargs =>
        cases target with
        | mk tname tneg targs =>
          simp only [Literal.mk.injEq]
          simp only at hname hneg hargs
          exact ⟨hname, hneg, hargs⟩

/-! ## matchLiteral produces ground substitution when target is ground -/

/-- If matchTerm succeeds against a ground target, the result stays ground. -/
private theorem matchTerm_ground_binding (σ : Substitution) (pat target : Term)
    (σ' : Substitution) (hgnd : target.isGround = true)
    (hmatch : matchTerm σ pat target = some σ')
    (hσgnd : σ.isGround = true) :
    σ'.isGround = true := by
  unfold matchTerm at hmatch
  split at hmatch
  · -- variable
    rename_i v
    cases hlook : σ.lookup v with
    | some t =>
      simp only [hlook] at hmatch
      have ⟨_, hσeq⟩ := if_some_eq_some hmatch
      subst hσeq; exact hσgnd
    | none =>
      simp only [hlook] at hmatch
      have hσeq : σ' = ⟨(v, target) :: σ.map⟩ := by cases hmatch; rfl
      subst hσeq
      simp only [Substitution.isGround, List.all_cons, Bool.and_eq_true]
      exact ⟨hgnd, hσgnd⟩
  · -- ground
    have ⟨_, hσeq⟩ := if_some_eq_some hmatch
    subst hσeq; exact hσgnd

/-- matchTerms against all-ground targets preserves ground substitution. -/
private theorem matchTerms_ground (σ : Substitution) (pats targets : List Term)
    (σ' : Substitution)
    (htgnd : targets.all Term.isGround = true)
    (hmatch : matchTerms σ pats targets = some σ')
    (hσgnd : σ.isGround = true) :
    σ'.isGround = true := by
  induction pats generalizing σ targets with
  | nil =>
    cases targets with
    | nil => simp [matchTerms] at hmatch; subst hmatch; exact hσgnd
    | cons _ _ => simp [matchTerms] at hmatch
  | cons p ps ih =>
    cases targets with
    | nil => simp [matchTerms] at hmatch
    | cons t ts =>
      simp only [matchTerms] at hmatch
      simp only [List.all_cons, Bool.and_eq_true] at htgnd
      cases hmid : matchTerm σ p t with
      | none => simp [hmid] at hmatch
      | some σ_mid =>
        simp [hmid] at hmatch
        exact ih σ_mid ts htgnd.2 hmatch
          (matchTerm_ground_binding σ p t σ_mid htgnd.1 hmid hσgnd)

/-- If `matchLiteral` succeeds against a ground target, the resulting substitution
    is ground (all bound values are ground terms). -/
theorem matchLiteral_ground (pat target : Literal) (σ : Substitution)
    (htgnd : target.isGround = true)
    (hmatch : matchLiteral pat target = some σ) :
    σ.isGround = true := by
  simp only [matchLiteral] at hmatch
  split at hmatch
  · simp at hmatch
  · split at hmatch
    · simp at hmatch
    · simp only [Literal.isGround] at htgnd
      exact matchTerms_ground Substitution.empty pat.args target.args σ htgnd hmatch rfl

/-! ## Convenience: matching with result type -/

/-- Result of a match attempt. -/
inductive MatchResult where
  | success (σ : Substitution)
  | nameMismatch
  | negMismatch
  | arityMismatch
  | termMismatch
  deriving Repr, Inhabited, BEq

/-- Match with detailed failure information. -/
def matchLiteralDetailed (pat target : Literal) : MatchResult :=
  if pat.name != target.name then .nameMismatch
  else if pat.negation != target.negation then .negMismatch
  else if pat.args.length != target.args.length then .arityMismatch
  else match matchTerms Substitution.empty pat.args target.args with
    | some σ => .success σ
    | none => .termMismatch

end Spindle.Arith
