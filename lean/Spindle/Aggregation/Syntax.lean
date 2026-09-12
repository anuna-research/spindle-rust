import Spindle.Arith.GroundLiteral
import Spindle.Arith.Types
import SpindleLean.Rule

/-!
# Rule syntax for dependency extraction

This is a typed rule-schema fragment, not an SPL parser. Predicate names are
static; arguments use the existing arithmetic Term/Substitution model. Modes are
retained, but dependency analysis conservatively groups all modes and polarities
of a predicate/arity. Aggregate patterns are single relations, as in PR #29.
Expression evaluation, temporal syntax, and range restriction are separate
obligations. Function calls are pure computations and cannot read relations.
-/
namespace Spindle.Aggregation

structure PredicateKey where
  name : String
  arity : Nat
  deriving DecidableEq, Repr

structure Pattern where
  atom : Arith.Literal
  modeName : Option String := none
  modeNegated : Bool := false
  deriving DecidableEq, Repr

/-- A conservative conflict domain, independent of arguments, polarity and mode. -/
def Pattern.key (p : Pattern) : PredicateKey := ⟨p.atom.name, p.atom.args.length⟩

def Pattern.complement (p : Pattern) : Pattern :=
  { p with atom := { p.atom with negation := !p.atom.negation } }

theorem Pattern.complement_key (p : Pattern) : p.complement.key = p.key := rfl

def Pattern.applySubst (subst : Arith.Substitution) (p : Pattern) : Pattern :=
  { p with atom := p.atom.applySubst subst }

/-- No argument substitution can introduce a new dependency domain. -/
theorem Pattern.applySubst_key (subst : Arith.Substitution) (p : Pattern) :
    (p.applySubst subst).key = p.key := by
  simp [applySubst, key, Arith.Literal.applySubst, Arith.Substitution.applyTerms]

theorem Pattern.change_mode_key (p : Pattern) (name : Option String) (negated : Bool) :
    ({ p with modeName := name, modeNegated := negated } : Pattern).key = p.key := rfl

inductive Expression where
  | term (value : Arith.Term)
  | call (name : String) (args : List Expression)
  deriving Repr

structure FoldExpression where
  resultVar : String
  seed : Option Expression
  reducer : String
  extract : Expression
  pattern : Pattern
  groupingVars : List String := []
  deriving Repr

inductive Condition where
  | logic (pattern : Pattern)
  | bind (resultVar : String) (expr : Expression)
  | compare (op : Arith.CmpOp) (lhs rhs : Expression)
  | fold (expr : FoldExpression)
  deriving Repr

/-- All heads are produced together; a multi-head rule is not a disjunction. -/
structure SchemaRule where
  label : String
  kind : RuleType
  body : List Condition
  head : Pattern
  additionalHeads : List Pattern := []
  deriving Repr

def SchemaRule.heads (rule : SchemaRule) : List Pattern := rule.head :: rule.additionalHeads

/-- An input occurrence and whether it requires a completed relation. Strong
negation in a logic pattern remains an ordinary dependency, not NAF. -/
def Condition.input : Condition → Option (Pattern × Bool)
  | .logic p => some (p, false)
  | .fold f => some (f.pattern, true)
  | .bind _ _ | .compare _ _ _ => none

structure AggregateProgram where
  rules : List SchemaRule
  priorities : List (String × String) := []
  deriving Repr

/-- Fail closed on predicate variables/wildcards; this fragment cannot infer
their domains. A name beginning with `_` is otherwise an ordinary static name. -/
def Pattern.hasStaticName (p : Pattern) : Bool :=
  match p.atom.name.toList with
  | [] => false
  | ['_'] => false
  | first :: _ => first != '?'

def SchemaRule.hasStaticNames (rule : SchemaRule) : Bool :=
  rule.heads.all Pattern.hasStaticName && rule.body.all (fun condition =>
    match condition.input with
    | none => true
    | some (p, _) => p.hasStaticName)

/-- Ordinary logical premises retained by a ground schema instance. -/
def SchemaRule.premises (r : SchemaRule) : List Pattern :=
  r.body.filterMap fun c => match c with | .logic p => some p | _ => none

end Spindle.Aggregation
