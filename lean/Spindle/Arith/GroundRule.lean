/-
  Spindle Ground Rule

  Defines the Rule type (head + body), substitution application over rules,
  groundness, and grounding (the set of all ground instances over a domain).
  Proves that applying a complete substitution to a rule produces a ground rule.
-/
import Spindle.Arith.GroundLiteral

namespace Spindle.Arith

/-! ## Rule type -/

/-- A rule in Spindle's logic language.
    Consists of a head literal and a list of body literals.
    Mirrors the Rust `Rule` struct's core fields relevant to grounding. -/
structure Rule where
  /-- The head (consequent) literal. -/
  head : Literal
  /-- The body (antecedent) literals. -/
  body : List Literal
  deriving Repr, Inhabited, BEq

/-! ## Groundness -/

/-- A rule is ground iff its head and all body literals are ground. -/
def Rule.isGround (r : Rule) : Bool :=
  r.head.isGround && r.body.all Literal.isGround

/-- A rule with ground head and no body is ground. -/
theorem Rule.isGround_fact (head : Literal) (hgnd : head.isGround = true) :
    (Rule.mk head []).isGround = true := by
  simp only [Rule.isGround, hgnd, List.all_nil, Bool.and_true]

/-- A rule with no body is ground iff its head is ground. -/
theorem Rule.isGround_no_body (head : Literal) :
    (Rule.mk head []).isGround = head.isGround := by
  simp only [Rule.isGround, List.all_nil, Bool.and_true]

/-! ## Variables in a rule -/

/-- Collect all variable names occurring in a rule (head + body). -/
def Rule.vars (r : Rule) : List String :=
  r.head.vars ++ r.body.flatMap Literal.vars

/-! ## Substitution application -/

/-- Apply a substitution to a rule, replacing variables in both head and body. -/
def Rule.applySubst (σ : Substitution) (r : Rule) : Rule :=
  { head := r.head.applySubst σ
    body := r.body.map (Literal.applySubst σ) }

/-- Applying a substitution preserves the number of body literals. -/
theorem Rule.applySubst_body_length (σ : Substitution) (r : Rule) :
    (r.applySubst σ).body.length = r.body.length := by
  simp only [Rule.applySubst, List.length_map]

/-- Applying the empty substitution is identity. -/
theorem Rule.applySubst_empty (r : Rule) :
    r.applySubst Substitution.empty = r := by
  simp only [Rule.applySubst]
  cases r with
  | mk head body =>
    simp only [Rule.mk.injEq]
    refine ⟨Literal.applySubst_empty head, ?_⟩
    induction body with
    | nil => rfl
    | cons l ls ih =>
      simp only [List.map_cons, List.cons.injEq]
      exact ⟨Literal.applySubst_empty l, ih⟩

/-- Applying a substitution to a rule with no body only affects the head. -/
theorem Rule.applySubst_no_body (σ : Substitution) (head : Literal) :
    (Rule.mk head []).applySubst σ = Rule.mk (head.applySubst σ) [] := rfl

/-! ## Completeness -/

/-- A substitution is complete for a rule if applying it grounds the head
    and every body literal. -/
def Substitution.completeRule (σ : Substitution) (r : Rule) : Bool :=
  σ.completeLiteral r.head && r.body.all σ.completeLiteral

/-! ## Helper: mapping substitution over body preserves all-ground -/

private theorem applySubst_body_all_ground (σ : Substitution) (ls : List Literal)
    (h : ls.all σ.completeLiteral = true) :
    (ls.map (Literal.applySubst σ)).all Literal.isGround = true := by
  induction ls with
  | nil => rfl
  | cons l ls ih =>
    simp only [List.map_cons, List.all_cons, Bool.and_eq_true] at h ⊢
    exact ⟨Literal.applySubst_ground σ l h.1, ih h.2⟩

/-! ## Main theorem: complete substitution produces ground rule -/

/-- **Main theorem**: applying a complete substitution to a rule produces
    a ground rule. A substitution is "complete" for a rule when it maps
    every variable in the rule (head + body) to a ground term. -/
theorem Rule.applySubst_ground (σ : Substitution) (r : Rule)
    (hcomplete : σ.completeRule r = true) :
    (r.applySubst σ).isGround = true := by
  simp only [Substitution.completeRule, Bool.and_eq_true] at hcomplete
  simp only [Rule.isGround, Rule.applySubst, Bool.and_eq_true]
  exact ⟨Literal.applySubst_ground σ r.head hcomplete.1,
         applySubst_body_all_ground σ r.body hcomplete.2⟩

/-! ## Covering corollary -/

/-- Helper: completeLiteral holds for all literals in a list when the substitution
    is ground and covers all variables in those literals. -/
private theorem completeLiteral_all_of_covering (σ : Substitution)
    (ls : List Literal) (hgnd : σ.isGround = true)
    (hcov : ∀ v, v ∈ ls.flatMap Literal.vars → σ.inDomain v = true) :
    ls.all σ.completeLiteral = true := by
  induction ls with
  | nil => rfl
  | cons l ls ih =>
    simp only [List.all_cons, Bool.and_eq_true]
    refine ⟨?_, ?_⟩
    · apply Substitution.complete_of_ground_covering σ l hgnd
      intro v hv
      apply hcov
      simp only [List.flatMap_cons, List.mem_append]
      exact Or.inl hv
    · apply ih
      intro v hv
      apply hcov
      simp only [List.flatMap_cons, List.mem_append]
      exact Or.inr hv

/-- **Corollary**: a ground substitution that covers all variables in a rule
    is complete for that rule. -/
theorem Substitution.complete_of_ground_covering_rule (σ : Substitution) (r : Rule)
    (hgnd : σ.isGround = true)
    (hcov : ∀ v, v ∈ r.vars → σ.inDomain v = true) :
    σ.completeRule r = true := by
  simp only [Substitution.completeRule, Bool.and_eq_true]
  refine ⟨?_, ?_⟩
  · apply Substitution.complete_of_ground_covering σ r.head hgnd
    intro v hv
    apply hcov
    simp only [Rule.vars, List.mem_append]
    exact Or.inl hv
  · apply completeLiteral_all_of_covering σ r.body hgnd
    intro v hv
    apply hcov
    simp only [Rule.vars, List.mem_append]
    exact Or.inr hv

/-! ## Grounding: all ground instances over a domain -/

/-- A domain is a finite set of ground terms that variables can be mapped to. -/
abbrev Domain := List Term

/-- All possible substitutions mapping a list of variables to domain values. -/
def allSubstitutions (vars : List String) (dom : Domain) : List Substitution :=
  match vars with
  | [] => [Substitution.empty]
  | v :: vs =>
    let rest := allSubstitutions vs dom
    dom.flatMap fun t => rest.map fun σ => σ.set v t

/-- The grounding of a rule over a domain: the set of all ground instances
    obtained by substituting domain values for every variable. -/
def Rule.groundInstances (r : Rule) (dom : Domain) : List Rule :=
  let vars := r.vars.eraseDups
  let subs := allSubstitutions vars dom
  subs.map (fun σ => r.applySubst σ)

/-- A ground rule is its own ground instance (up to the identity substitution). -/
theorem Rule.applySubst_empty_eq (r : Rule) :
    r.applySubst Substitution.empty = r :=
  Rule.applySubst_empty r

end Spindle.Arith
