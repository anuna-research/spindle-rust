import SpindleLean.Literal

/-!
# SpindleLean.Rule

Defines the `RuleType` inductive and the `Rule` structure for defeasible logic.

## Rust correspondence

`RuleType` corresponds to `crates/spindle-core/src/rule.rs :: RuleType`.

| Lean variant   | Rust variant          | Arrow | Notes                              |
|---------------|-----------------------|-------|------------------------------------|
| `strict`      | `RuleType::Strict`    | `->`  | Cannot be defeated                 |
| `defeasible`  | `RuleType::Defeasible`| `=>`  | Can be defeated by opposing rules  |
| `defeater`    | `RuleType::Defeater`  | `~>`  | Blocks conclusions, proves nothing |

The Rust enum also has a `Fact` variant (arrow `>>`), which represents
unconditional truths with no antecedents.  In the Lean formalisation, facts are
modelled as strict rules with an empty body rather than a separate variant,
keeping the inductive simpler for proof purposes.

`Rule` corresponds to `crates/spindle-core/src/rule.rs :: Rule`, stripped to
the four fields relevant for propositional DL(d) verification:

| Lean field   | Rust field    | Notes                                          |
|-------------|---------------|-------------------------------------------------|
| `label`     | `label`       | Unique rule identifier (plain `String` here)    |
| `ruleType`  | `rule_type`   | Strict / defeasible / defeater                  |
| `body`      | `body`        | Antecedent literals (`List Literal` here)       |
| `head`      | `head`        | Single consequent literal                       |

The Rust struct carries additional fields (`template_label`, `mode`, `temporal`,
`constraints`) that are orthogonal to the core defeasible-logic properties
verified here and are therefore omitted.
-/

/-- The type of a rule, determining its strength in defeasible logic. -/
inductive RuleType where
  /-- Strict rule (`->`): conclusion follows necessarily when body is satisfied. -/
  | strict
  /-- Defeasible rule (`=>`): conclusion follows defeasibly; can be defeated. -/
  | defeasible
  /-- Defeater (`~>`): blocks opposing conclusions but does not prove anything. -/
  | defeater
  deriving DecidableEq, BEq, Hashable, Repr

namespace RuleType

/-- The arrow symbol for a rule type, matching the Rust `arrow()` method. -/
def arrow : RuleType → String
  | strict     => "->"
  | defeasible => "=>"
  | defeater   => "~>"

/-- A strict rule produces definite conclusions. -/
def isDefinite : RuleType → Bool
  | strict => true
  | _      => false

/-- A defeasible rule produces defeasible conclusions. -/
def isDefeasible : RuleType → Bool
  | defeasible => true
  | _          => false

/-- A defeater blocks but does not prove. -/
def isDefeater : RuleType → Bool
  | defeater => true
  | _        => false

instance : ToString RuleType where
  toString rt := rt.arrow

end RuleType

/-- A rule in defeasible logic: a labelled, typed implication from body literals
    to a single head literal. -/
structure Rule where
  /-- Unique label identifying this rule. -/
  label    : String
  /-- The strength of the rule. -/
  ruleType : RuleType
  /-- Antecedent (body) literals — empty for facts. -/
  body     : List Literal
  /-- Consequent (head) literal. -/
  head     : Literal
  deriving DecidableEq, BEq, Repr

namespace Rule

/-- A rule is a fact when it is strict with an empty body. -/
def isFact (r : Rule) : Bool :=
  r.ruleType == .strict && r.body.isEmpty

/-- Convenience constructor for a strict rule. -/
def mkStrict (label : String) (body : List Literal) (head : Literal) : Rule :=
  { label, ruleType := .strict, body, head }

/-- Convenience constructor for a defeasible rule. -/
def mkDefeasible (label : String) (body : List Literal) (head : Literal) : Rule :=
  { label, ruleType := .defeasible, body, head }

/-- Convenience constructor for a defeater. -/
def mkDefeater (label : String) (body : List Literal) (head : Literal) : Rule :=
  { label, ruleType := .defeater, body, head }

/-- Convenience constructor for a fact (strict rule with empty body). -/
def mkFact (label : String) (head : Literal) : Rule :=
  { label, ruleType := .strict, body := [], head }

end Rule
