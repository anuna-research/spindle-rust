import SpindleLean.Rule

/-!
# SpindleLean.Theory

Defines the `Theory` structure for defeasible logic: a collection of rules
together with a superiority relation over rule labels.

## Rust correspondence

`Theory` corresponds to `crates/spindle-core/src/theory.rs :: Theory`, stripped
to the two fields relevant for propositional DL(d) verification:

| Lean field       | Rust field        | Notes                                         |
|-----------------|-------------------|-----------------------------------------------|
| `rules`         | `rules`           | All rules in the theory (`List Rule` here)    |
| `superiority`   | `superiorities`   | Pairs `(l₁, l₂)` meaning `l₁ > l₂`          |

The Rust struct carries additional fields (`sup_index`, `metadata`,
`label_counter`, `trust_policy`) that are either derived caches or orthogonal to
the core defeasible-logic properties verified here and are therefore omitted.

The `sup` helper corresponds to the Rust method `Theory::is_superior`, providing
a Boolean check of whether one rule is declared superior to another.
-/

/-- A defeasible theory: a set of rules and a superiority relation.

The superiority relation is a list of pairs `(l₁, l₂)` of rule labels,
where each pair asserts that the rule labelled `l₁` is superior to the
rule labelled `l₂`. -/
structure Theory where
  /-- The rules comprising this theory. -/
  rules       : List Rule
  /-- The superiority relation: `(l₁, l₂)` means rule `l₁` is superior to `l₂`. -/
  superiority : List (String × String)
  deriving Repr

namespace Theory

/-- Check whether rule `r₁` is declared superior to rule `r₂` in theory `t`.

Corresponds to `Theory::is_superior` in Rust, which performs an O(1) lookup via
`sup_index`. Here we use a simple list membership check, which is equivalent for
verification purposes. -/
def sup (t : Theory) (r₁ r₂ : Rule) : Bool :=
  t.superiority.any fun (l₁, l₂) => l₁ == r₁.label && l₂ == r₂.label

/-- Look up a rule by its label. -/
def ruleByLabel (t : Theory) (label : String) : Option Rule :=
  t.rules.find? fun r => r.label == label

/-- All strict rules in the theory. -/
def strictRules (t : Theory) : List Rule :=
  t.rules.filter fun r => r.ruleType == .strict

/-- All defeasible rules in the theory. -/
def defeasibleRules (t : Theory) : List Rule :=
  t.rules.filter fun r => r.ruleType == .defeasible

/-- All defeater rules in the theory. -/
def defeaterRules (t : Theory) : List Rule :=
  t.rules.filter fun r => r.ruleType == .defeater

/-- All facts (strict rules with empty body) in the theory. -/
def facts (t : Theory) : List Rule :=
  t.rules.filter fun r => r.isFact

/-- All rules whose head matches the given literal. -/
def rulesForHead (t : Theory) (l : Literal) : List Rule :=
  t.rules.filter fun r => r.head == l

/-- `R` — all rules whose head matches literal `q`.
    Alias for `rulesForHead`, matching the notation R(theory, q) from the literature. -/
abbrev R := rulesForHead

/-- `Rs` — all strict rules whose head matches literal `q`. -/
def Rs (t : Theory) (q : Literal) : List Rule :=
  t.rules.filter fun r => r.head == q && r.ruleType == .strict

/-- `Rsd` — all strict or defeasible rules whose head matches literal `q`. -/
def Rsd (t : Theory) (q : Literal) : List Rule :=
  t.rules.filter fun r => r.head == q && (r.ruleType == .strict || r.ruleType == .defeasible)

end Theory
