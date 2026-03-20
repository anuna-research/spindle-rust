/-!
# SpindleLean.Literal

Defines the `Literal` type: an atom name (`String`) paired with a polarity (`Bool`).

## Rust correspondence

This type corresponds to `crates/spindle-core/src/literal.rs :: Literal`, stripped to the
two fields relevant for propositional DL(d) verification:

| Lean field   | Rust field              | Notes                                    |
|-------------|-------------------------|------------------------------------------|
| `name`      | `name_id` / `name()`   | Atom name (interned in Rust, plain here) |
| `polarity`  | `negation`              | `true` = positive, `false` = negated     |

The Rust struct carries additional fields (`mode`, `temporal`, `temporal_expr`,
`predicate_args`) that are orthogonal to the core defeasible-logic properties
verified here and are therefore omitted.

Note on polarity convention: Rust stores a `negation: bool` where `true` means
negated. We invert this to `polarity: Bool` where `true` means positive, which
reads more naturally in Lean proofs (e.g., `lit.polarity` for "the literal is
affirmed").
-/

/-- A literal is an atom name paired with a polarity.
    `polarity = true` means the literal is positive (not negated).
    `polarity = false` means the literal is negated. -/
structure Literal where
  name     : String
  polarity : Bool
  deriving DecidableEq, BEq, Hashable, Repr

namespace Literal

/-- Construct a positive literal. -/
def pos (name : String) : Literal :=
  { name, polarity := true }

/-- Construct a negated literal. -/
def neg (name : String) : Literal :=
  { name, polarity := false }

/-- The complement (negation) of a literal. -/
def complement (l : Literal) : Literal :=
  { l with polarity := !l.polarity }

/-- Two literals are complementary iff they share a name but differ in polarity. -/
def isComplement (l₁ l₂ : Literal) : Bool :=
  l₁.name == l₂.name && l₁.polarity != l₂.polarity

theorem complement_involution (l : Literal) : complement (complement l) = l := by
  cases l
  simp [complement, Bool.not_not]

/-- Alias using dot notation. -/
theorem complement_involutive (l : Literal) : l.complement.complement = l :=
  complement_involution l

theorem complement_name (l : Literal) : l.complement.name = l.name := by
  simp [complement]

theorem complement_polarity (l : Literal) : l.complement.polarity = !l.polarity := by
  simp [complement]

instance : ReflBEq Literal where
  rfl {a} := by
    cases a with
    | mk n p =>
      simp [instBEqLiteral.beq, BEq.beq]

instance : LawfulBEq Literal where
  eq_of_beq {a b} h := by
    cases a with
    | mk na pa =>
      cases b with
      | mk nb pb =>
        simp [instBEqLiteral.beq, BEq.beq] at h
        obtain ⟨h1, h2⟩ := h
        subst h1; subst h2; rfl

end Literal
