/-
  Spindle Substitution Type

  Defines substitutions as partial maps from variable names to ground terms
  (SPEC-017). Provides application, composition, domain, and occurs-check
  operations with key correctness properties.
-/
import Spindle.Arith.Term

namespace Spindle.Arith

/-! ## Groundness -/

/-- A term is ground iff it contains no variables. -/
def Term.isGround : Term → Bool
  | .variable _ => false
  | _ => true

theorem Term.isGround_symbol (s : String) : (Term.symbol s).isGround = true := rfl
theorem Term.isGround_integer (n : Int) : (Term.integer n).isGround = true := rfl
theorem Term.isGround_decimal (n : Int) (s : Nat) : (Term.decimal n s).isGround = true := rfl
theorem Term.isGround_float (f : Float) : (Term.finiteFloat f).isGround = true := rfl
theorem Term.isGround_variable (v : String) : (Term.variable v).isGround = false := rfl

/-! ## Substitution type -/

/-- A substitution is a finite partial map from variable names to terms.
    Internally represented as an association list. -/
structure Substitution where
  /-- The underlying mapping from variable names to terms. -/
  map : List (String × Term)
  deriving Repr, Inhabited

instance : BEq Substitution where
  beq a b := a.map == b.map

/-- The empty substitution. -/
def Substitution.empty : Substitution := ⟨[]⟩

/-- Singleton substitution mapping one variable to a term. -/
def Substitution.singleton (v : String) (t : Term) : Substitution := ⟨[(v, t)]⟩

/-! ## Lookup and domain -/

/-- Look up a variable in the substitution. Returns the first binding found. -/
def Substitution.lookup (σ : Substitution) (v : String) : Option Term :=
  σ.map.lookup v

/-- The domain of a substitution: the set of bound variable names. -/
def Substitution.domain (σ : Substitution) : List String :=
  σ.map.map Prod.fst

/-- Check whether a variable is in the domain. -/
def Substitution.inDomain (σ : Substitution) (v : String) : Bool :=
  σ.lookup v |>.isSome

/-- Number of bindings. -/
def Substitution.size (σ : Substitution) : Nat :=
  σ.map.length

/-! ## Application -/

/-- Apply a substitution to a single term.
    Variables are replaced by their binding if present;
    all other terms are returned unchanged. -/
def Substitution.applyTerm (σ : Substitution) : Term → Term
  | .variable v => match σ.lookup v with
    | some t => t
    | none => .variable v
  | t => t

/-- Apply a substitution to a list of terms. -/
def Substitution.applyTerms (σ : Substitution) (ts : List Term) : List Term :=
  ts.map σ.applyTerm

/-! ## Occurs-check -/

/-- Does variable `v` occur in term `t`?
    For our flat term language (no nested function symbols),
    this simply checks whether `t` is exactly `variable v`. -/
def Term.occurs (v : String) : Term → Bool
  | .variable w => v == w
  | _ => false

/-- Occurs-check: does variable `v` occur in any term in the range of `σ`? -/
def Substitution.occursInRange (σ : Substitution) (v : String) : Bool :=
  σ.map.any fun (_, t) => t.occurs v

/-- A substitution is safe (passes occurs-check) if no domain variable
    occurs in the range of the substitution. -/
def Substitution.isSafe (σ : Substitution) : Bool :=
  σ.map.all fun (v, _) => !σ.occursInRange v

/-! ## Extension and composition -/

/-- Extend a substitution with a new binding.
    If the variable is already bound, the old binding is kept (no override). -/
def Substitution.extend (σ : Substitution) (v : String) (t : Term) : Substitution :=
  if σ.inDomain v then σ else ⟨(v, t) :: σ.map⟩

/-- Extend with override: always adds/replaces the binding for `v`. -/
def Substitution.set (σ : Substitution) (v : String) (t : Term) : Substitution :=
  ⟨(v, t) :: σ.map.filter (fun p => p.1 != v)⟩

/-- Compose two substitutions: (σ₁ ∘ σ₂) applied to a variable first applies σ₂,
    then applies σ₁ to the result. The composed substitution is built by:
    1. Applying σ₁ to each term in σ₂'s range
    2. Adding any bindings from σ₁ whose variables are not in σ₂'s domain -/
def Substitution.compose (σ₁ σ₂ : Substitution) : Substitution :=
  let applied := σ₂.map.map fun (v, t) => (v, σ₁.applyTerm t)
  let extra := σ₁.map.filter fun (v, _) => !σ₂.inDomain v
  ⟨applied ++ extra⟩

instance : Append Substitution where
  append σ₁ σ₂ := Substitution.compose σ₁ σ₂

/-! ## Ground substitution -/

/-- A substitution is ground if every term in its range is ground. -/
def Substitution.isGround (σ : Substitution) : Bool :=
  σ.map.all fun (_, t) => t.isGround

/-! ## Properties -/

/-- The empty substitution has empty domain. -/
theorem Substitution.domain_empty : Substitution.empty.domain = [] := rfl

/-- The empty substitution has size 0. -/
theorem Substitution.size_empty : Substitution.empty.size = 0 := rfl

/-- Applying the empty substitution is identity. -/
theorem Substitution.applyTerm_empty (t : Term) :
    Substitution.empty.applyTerm t = t := by
  cases t <;> rfl

/-- Looking up a variable not in the domain returns none. -/
theorem Substitution.lookup_empty (v : String) :
    Substitution.empty.lookup v = none := rfl

/-- A singleton maps its variable correctly. -/
theorem Substitution.lookup_singleton (v : String) (t : Term) :
    (Substitution.singleton v t).lookup v = some t := by
  simp only [singleton, lookup, List.lookup]
  simp [BEq.beq]

/-- Applying a singleton replaces the bound variable. -/
theorem Substitution.applyTerm_singleton_hit (v : String) (t : Term) :
    (Substitution.singleton v t).applyTerm (.variable v) = t := by
  simp only [applyTerm, lookup_singleton]

/-- Applying a singleton leaves other variables unchanged. -/
theorem Substitution.applyTerm_singleton_miss (v w : String) (t : Term)
    (h : v ≠ w) :
    (Substitution.singleton w t).applyTerm (.variable v) = .variable v := by
  simp only [applyTerm, singleton, lookup, List.lookup]
  simp [BEq.beq, h]

/-- Applying any substitution to a non-variable returns the term unchanged. -/
theorem Substitution.applyTerm_symbol (σ : Substitution) (s : String) :
    σ.applyTerm (.symbol s) = .symbol s := rfl

theorem Substitution.applyTerm_integer (σ : Substitution) (n : Int) :
    σ.applyTerm (.integer n) = .integer n := rfl

theorem Substitution.applyTerm_decimal (σ : Substitution) (n : Int) (s : Nat) :
    σ.applyTerm (.decimal n s) = .decimal n s := rfl

theorem Substitution.applyTerm_float (σ : Substitution) (f : Float) :
    σ.applyTerm (.finiteFloat f) = .finiteFloat f := rfl

/-- A variable does not occur in a non-variable term. -/
theorem Term.occurs_symbol (v : String) (s : String) :
    (Term.symbol s).occurs v = false := rfl

theorem Term.occurs_integer (v : String) (n : Int) :
    (Term.integer n).occurs v = false := rfl

/-- A variable occurs in itself. -/
theorem Term.occurs_self (v : String) :
    (Term.variable v).occurs v = true := by
  simp [Term.occurs, BEq.beq]

/-- The empty substitution is trivially safe. -/
theorem Substitution.isSafe_empty : Substitution.empty.isSafe = true := rfl

/-- The empty substitution is trivially ground. -/
theorem Substitution.isGround_empty : Substitution.empty.isGround = true := rfl

end Spindle.Arith
