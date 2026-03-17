# Trit Literal Refactor: Three-Valued Truth for Defeasible Reasoning

| Field | Value |
|---|---|
| Document ID | SPEC-019 |
| Title | Trit Literal Refactor: Three-Valued Truth for Defeasible Reasoning |
| Version | 1.0.0 |
| Status | Draft |
| Created | 2026-03-17 |
| Last Updated | 2026-03-17 |
| Authors | Claude (AI agent) |
| Reviewers | Core Maintainers |
| Protocol | [USDD Agent Protocol v1.3.0](../../handbook/engineering/usdd-agent-protocol.md) |

---

## 1. Executive Summary

Spindle currently represents literal polarity as a `bool` (`negation: bool` on `Literal`, `negated: bool` on `LiteralStruct`, and `is_negated()` on `LitId`). This two-valued representation forces every literal into one of two states — *asserted* or *negated* — which is inadequate for modelling **incomplete information** in defeasible reasoning.

This specification replaces the boolean polarity field with a **trit** — a balanced ternary value drawn from `{Negative, Unknown, Positive}` (equivalently `{-1, 0, +1}`). The third value, `Unknown`, represents a literal whose truth status has not yet been determined by the reasoning engine, as distinct from a literal that has been explicitly negated.

### Theoretical Foundation

The trit representation is grounded in **Strong Kleene three-valued logic** (K3), introduced by Kleene (1952) in *Introduction to Metamathematics*. In K3, the third value represents incomplete information — "we don't know yet but there is a definite answer." This is precisely the semantics required during forward-chaining in defeasible logic, where a literal may be neither proven nor disproven at any given point in the fixed-point computation.

The connection between three-valued logic and logic programming is established by Fitting (1991), "Bilattices and the Semantics of Logic Programming," which shows that Belnap's four-valued lattice — collapsing to Kleene's three-valued logic when contradictions are excluded — provides the correct fixpoint semantics for logic programs with negation-as-failure (NAF). This is exactly what a defeasible reasoner with NAF needs.

The algebraic operations on trits correspond to Kleene's truth tables:

| Spindle Operation | Kleene Equivalent | Algebraic Form |
|---|---|---|
| `Trit::not()` | Kleene negation | `-a` |
| `Trit::and()` | Kleene conjunction | `min(a, b)` in ordering `-1 < 0 < +1` |
| `Trit::or()` | Kleene disjunction | `max(a, b)` in ordering `-1 < 0 < +1` |
| `Trit::cmp_superiority()` | No Kleene analog | `sign(a - b)` — superiority comparison |

### Scope

| In Scope | Out of Scope |
|---|---|
| `Trit` type replacing `bool` for literal polarity | Belnap four-valued logic (FOUR) |
| Kleene conjunction, disjunction, negation | Paraconsistent reasoning (Priest's LP) |
| `Unknown` propagation through body evaluation | Constraint logic programming |
| Updated `LitId` bit-packing for ternary states | Changes to `ConclusionType` or proof tag system |
| Backward-compatible serialisation with `serde` | Changes to SPL surface syntax |
| Updated `LiteralBitSet` for three-valued tracking | Well-founded semantics (WFS) |

---

## 2. Motivation and Context

### 2.1 Current Limitations

#### Closed-World Assumption Baked Into Representation

The `negation: bool` field forces every literal into an assertion/negation dichotomy at construction time. Consider the reasoning engine during Phase 2 (defeasible forward chaining):

```
Given theory:
  r1: bird(X) => flies(X)
  r2: penguin(X) => ~flies(X)
  r1 > r2
  fact: bird(tweety)
```

During Phase 2, before any defeasible rules fire, what is the status of `flies(tweety)`? It is *neither asserted nor negated* — it is **unknown**. But the current `bool` representation forces `Literal::simple("flies")` to carry `negation: false`, conflating "positive polarity" with "asserted truth."

This conflation is harmless when the reasoning engine mediates all truth via `LiteralBitSet` membership, but it creates semantic impedance in three scenarios:

1. **Extension interfaces**: External consumers querying the theory mid-reasoning cannot distinguish "not yet evaluated" from "positively asserted."
2. **Ambiguity propagation**: The ambiguity blocking algorithm (Condition 3 in +d proof) must currently use side-channel state (`rule_discarded` flags) to track what is effectively an "unknown" status for body literals.
3. **Incremental reasoning**: Future support for theory update and incremental re-reasoning requires distinguishing retracted literals (now unknown) from negated literals (actively denied).

#### Semantic Gap With the Literature

The proof tag system (`+D`/`+d`/`-D`/`-d`) from Antoniou et al. (2001) implicitly defines a three-valued structure over literals: *definitely provable*, *definitely not provable*, and *not yet determined*. The `bool` representation cannot encode this third state at the literal level, pushing it into auxiliary data structures (`LiteralBitSet` membership tests, `FxHashMap<&str, bool>` flags).

### 2.2 Design Constraints

1. **No change to ConclusionType.** The `+D`/`+d`/`-D`/`-d` tags operate at a higher level than literal polarity. A `+D ~flies(tweety)` is a *definite proof of the negation of flies(tweety)* — the negation is the literal's polarity, while `+D` is the proof strength. These remain orthogonal.

2. **No change to SPL syntax.** Users still write `~p` for negated literals and `p` for positive literals. The `Unknown` state is an internal engine state, not a user-facing concept in the current language.

3. **Balanced ternary arithmetic.** The `Trit` type uses the balanced ternary encoding `{-1, 0, +1}` as an `i8`. This is the standard encoding from Łukasiewicz (1920) and maps directly to the Kleene truth tables via clamped arithmetic.

4. **Backward-compatible bitwise operations.** The current `LitId` packs negation into bit 31. The new encoding must maintain O(1) complement and membership operations.

### 2.3 Academic References

| Reference | Relevance |
|---|---|
| Łukasiewicz, J. (1920). "O logice trójwartościowej." | First formal three-valued logic; third value = "possible" for future contingents |
| Kleene, S.C. (1952). *Introduction to Metamathematics*. | Strong Kleene (K3) three-valued truth tables — the specific logic this spec implements |
| Belnap, N. (1977). "A Useful Four-Valued Logic." | Lattice structure `{⊥, t, f, ⊤}` that collapses to K3 when contradictions are excluded |
| Priest, G. (1979). "The Logic of Paradox." | LP: third value = "both true and false" (paraconsistent) — different motivation, same ∧/∨ tables |
| Fitting, M. (1985, 1991). "Bilattices and the Semantics of Logic Programming." | Bridge between Belnap's lattice and Datalog-style forward chaining with fixpoints |
| Nute, D. (1994). "Defeasible Logic." *Handbook of Logic in AI and Logic Programming*. | Original defeasible logic formalization |
| Antoniou, G., Billington, D., Governatori, G., Maher, M.J. (2001). "Representation Results for Defeasible Logic." | Formalizes `+D`/`+d`/`-D`/`-d` tags as a lattice — a three-valued structure |
| Maher, M.J. (2001). "Propositional Defeasible Logic has Linear Complexity." | Complexity bound that makes SPINdle's forward chaining practical |

---

## 3. Functional Requirements

### REQ-001: Trit Type Definition

The system SHALL provide a `Trit` type representing balanced ternary truth values with exactly three variants:

- `Positive` (+1): The literal is asserted / true.
- `Unknown` (0): The literal's truth status is indeterminate.
- `Negative` (-1): The literal is negated / false.

The `Trit` type SHALL be represented as an `i8` internally, using the balanced ternary encoding: `Positive = 1`, `Unknown = 0`, `Negative = -1`.

The `Trit` type SHALL derive or implement: `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, `Ord`, `Default` (defaulting to `Unknown`).

Trace:
- TEST-001
- CON-001

---

### REQ-002: Kleene Negation

The system SHALL implement negation on `Trit` following Kleene's truth table:

| Input | Output |
|---|---|
| `Positive` | `Negative` |
| `Unknown` | `Unknown` |
| `Negative` | `Positive` |

Algebraically: `not(a) = -a`.

This SHALL be exposed as `Trit::not()` and via `impl std::ops::Not for Trit`.

Trace:
- TEST-002
- CON-001

---

### REQ-003: Kleene Conjunction

The system SHALL implement conjunction on `Trit` following Strong Kleene's truth table:

| a \ b | Positive | Unknown | Negative |
|---|---|---|---|
| **Positive** | Positive | Unknown | Negative |
| **Unknown** | Unknown | Unknown | Negative |
| **Negative** | Negative | Negative | Negative |

Algebraically: `and(a, b) = min(a, b)` in the ordering `Negative < Unknown < Positive`.

This SHALL be exposed as `Trit::and()` and via `impl std::ops::BitAnd for Trit`.

Trace:
- TEST-003
- CON-001

---

### REQ-004: Kleene Disjunction

The system SHALL implement disjunction on `Trit` following Strong Kleene's truth table:

| a \ b | Positive | Unknown | Negative |
|---|---|---|---|
| **Positive** | Positive | Positive | Positive |
| **Unknown** | Positive | Unknown | Unknown |
| **Negative** | Positive | Unknown | Negative |

Algebraically: `or(a, b) = max(a, b)` in the ordering `Negative < Unknown < Positive`.

This SHALL be exposed as `Trit::or()` and via `impl std::ops::BitOr for Trit`.

Trace:
- TEST-004
- CON-001

---

### REQ-005: Kleene Implication

The system SHALL implement material implication on `Trit` following Strong Kleene's truth table:

| a → b | Positive | Unknown | Negative |
|---|---|---|---|
| **Positive** | Positive | Unknown | Negative |
| **Unknown** | Positive | Unknown | Unknown |
| **Negative** | Positive | Positive | Positive |

Algebraically: `implies(a, b) = max(-a, b)`.

This SHALL be exposed as `Trit::implies()`.

Trace:
- TEST-005
- CON-001

---

### REQ-006: Trit Predicates

The system SHALL provide the following predicate methods on `Trit`:

- `is_positive()` → `bool`: Returns `true` iff the value is `Positive`.
- `is_negative()` → `bool`: Returns `true` iff the value is `Negative`.
- `is_unknown()` → `bool`: Returns `true` iff the value is `Unknown`.
- `is_determined()` → `bool`: Returns `true` iff the value is NOT `Unknown` (i.e., `Positive` or `Negative`).

Trace:
- TEST-006
- CON-001

---

### REQ-007: Trit ↔ Bool Conversion

The system SHALL provide conversion between `Trit` and `bool`:

- `From<bool> for Trit`: `true` → `Positive`, `false` → `Negative`.
- `Trit::to_bool()` → `Option<bool>`: `Positive` → `Some(true)`, `Negative` → `Some(false)`, `Unknown` → `None`.
- `Trit::to_bool_or(default: bool)` → `bool`: Like `to_bool()` but returns `default` for `Unknown`.

These conversions enable backward compatibility at API boundaries where `bool` is expected.

Trace:
- TEST-007
- CON-001

---

### REQ-008: Trit ↔ i8 Conversion

The system SHALL provide conversion between `Trit` and `i8`:

- `From<Trit> for i8`: `Positive` → `1`, `Unknown` → `0`, `Negative` → `-1`.
- `TryFrom<i8> for Trit`: Values `-1`, `0`, `1` succeed; all others return an error.
- `Trit::from_i8_clamped(v: i8)` → `Trit`: Clamps any value `< 0` to `Negative`, `0` to `Unknown`, `> 0` to `Positive`.

Trace:
- TEST-008
- CON-001

---

### REQ-009: Literal Polarity as Trit

The system SHALL replace the `negation: bool` field on `Literal` with a `polarity: Trit` field.

The existing API surface SHALL be preserved via deprecated compatibility methods:

- `Literal::is_negated()` → `bool`: Returns `self.polarity.is_negative()`. Marked `#[deprecated]`.
- `Literal::negation` field access: Replaced by `Literal::polarity`.

New methods:

- `Literal::polarity()` → `Trit`: Returns the literal's polarity.
- `Literal::is_positive()` → `bool`: Returns `self.polarity.is_positive()`.
- `Literal::is_unknown()` → `bool`: Returns `self.polarity.is_unknown()`.
- `Literal::unknown(name: impl AsRef<str>)` → `Literal`: Constructor producing a literal with `Unknown` polarity.

The `Literal::complement()` method SHALL use `Trit::not()`, which correctly maps `Unknown` → `Unknown`.

Trace:
- TEST-009
- CON-002

---

### REQ-010: LiteralStruct Polarity as Trit

The system SHALL replace the `negated: bool` field on `LiteralStruct` with a `polarity: Trit` field.

Serde serialization SHALL represent `Trit` as the string `"positive"`, `"unknown"`, or `"negative"` for JSON readability.

Trace:
- TEST-010
- CON-002

---

### REQ-011: LitId Ternary Encoding

The system SHALL update `LitId` to encode ternary polarity using 2 bits instead of the current 1-bit negation flag:

| Bits [31:30] | Meaning |
|---|---|
| `00` | Positive |
| `01` | Unknown |
| `10` | Negative |
| `11` | Reserved (unused; SHALL panic on construction in debug builds) |

The atom capacity drops from 2³¹ to 2³⁰ (≈ 1 billion atoms). This is acceptable for all practical defeasible theories.

The following operations SHALL maintain O(1) time complexity:
- `LitId::new(atom: AtomId, polarity: Trit)` → `LitId`
- `LitId::polarity()` → `Trit`
- `LitId::atom()` → `AtomId`
- `LitId::complement()` → `LitId`: Flips `Positive` ↔ `Negative`, preserves `Unknown`.

The existing `LitId::is_negated()` method SHALL be preserved as a deprecated alias for `self.polarity().is_negative()`.

Trace:
- TEST-011
- CON-003

---

### REQ-012: LiteralBitSet Ternary Extension

The system SHALL update `LiteralBitSet` to support three-valued tracking per literal.

Each atom SHALL use 3 bit positions: positive, unknown, negated. The monotonicity invariant (no `remove`) is preserved.

The following operations SHALL be provided:
- `contains(id: LitId)` → `bool`: Checks if the literal with the given polarity has been inserted.
- `insert(id: LitId)`: Marks the literal with the given polarity.
- `contains_any_polarity(atom: AtomId)` → `bool`: Returns `true` if any polarity of the atom has been inserted.
- `polarity_of(atom: AtomId)` → `Option<Trit>`: Returns the polarity of the atom if exactly one polarity is set, `None` if zero or multiple are set.

Trace:
- TEST-012
- CON-004

---

### REQ-013: Reasoning State Unknown Propagation

The system SHALL use `Unknown` polarity during Phase 2 (defeasible forward chaining) to explicitly represent literals whose truth status has not yet been determined by the fixed-point computation.

Specifically:
- When a body literal of a defeasible rule has not been proven `+d` or disproven `-d`, its contribution to body evaluation SHALL be `Unknown` rather than the current implicit "not yet seen."
- The `rule_discarded: FxHashMap<&str, bool>` tracking map SHALL be replaced by a `Trit`-valued map: `rule_body_status: FxHashMap<&str, Trit>`, where `Negative` means a body literal was disproven, `Unknown` means no body literal was disproven but not all are proven, and `Positive` means all body literals are proven.
- The fixed-point loop SHALL terminate when no literal's polarity changes between iterations (monotonicity is preserved because polarities only move from `Unknown` → `Positive` or `Unknown` → `Negative`, never backward).

Trace:
- TEST-013
- CON-005

---

### REQ-014: Display and Debug Formatting

The system SHALL display `Trit` values as follows:

- `Display`: `"+"` for Positive, `"?"` for Unknown, `"-"` for Negative.
- `Debug`: `"Positive"`, `"Unknown"`, `"Negative"`.

When displaying a `Literal`:
- A literal with `Unknown` polarity SHALL be displayed with a `?` prefix (e.g., `?flies(tweety)`).
- A literal with `Negative` polarity SHALL continue to use the `~` prefix (e.g., `~flies(tweety)`).
- A literal with `Positive` polarity SHALL have no prefix (e.g., `flies(tweety)`).

Trace:
- TEST-014

---

### REQ-015: Serde Backward Compatibility

The system SHALL support deserializing theories serialized with the old `negation: bool` schema:

- `"negation": true` SHALL deserialize to `polarity: Trit::Negative`.
- `"negation": false` SHALL deserialize to `polarity: Trit::Positive`.

New serializations SHALL use the `polarity` field with string values (`"positive"`, `"unknown"`, `"negative"`).

Trace:
- TEST-015

---

## 4. Non-Functional Requirements

### NFR-001: Memory Footprint

The `Trit` type SHALL occupy exactly 1 byte (`size_of::<Trit>() == 1`). The `Literal` struct size SHALL not increase by more than 0 bytes (the `negation: bool` field already occupies 1 byte due to alignment; replacing it with `Trit` as `i8` is size-neutral).

Trace:
- TEST-016

---

### NFR-002: Operation Performance

All `Trit` logical operations (`not`, `and`, `or`, `implies`) SHALL execute in constant time without branching. The balanced ternary arithmetic encoding (`min`, `max`, `-a`) enables branch-free implementations using integer min/max intrinsics.

Trace:
- TEST-017

---

### NFR-003: No Regression in Reasoning Performance

The refactored reasoning engine SHALL produce identical conclusions to the current engine for all theories expressible in SPL. Performance regression SHALL not exceed 5% on the existing benchmark suite (measured as wall-clock time on the same hardware).

Trace:
- TEST-018

---

## 5. Architecture Decisions

### ADR-001: Strong Kleene (K3) Over Weak Kleene or LP

**Decision:** Use Strong Kleene three-valued logic for `Unknown` propagation.

**Context:** Three competing three-valued logics were considered:

| Logic | Third value semantics | NOT(Unknown) | AND(T, Unknown) |
|---|---|---|---|
| Strong Kleene (K3) | "Don't know yet" | Unknown | Unknown |
| Weak Kleene | "Meaningless/undefined" | Unknown | Unknown (but AND(F, Unknown) = Unknown too) |
| Priest's LP | "Both true and false" | Unknown | Unknown |

**Trade-off analysis:**

- **Weak Kleene** is too aggressive: `AND(False, Unknown) = Unknown` rather than `False`. This means a rule body containing a disproven literal and an unknown literal would evaluate to `Unknown` rather than `False`, preventing the engine from discarding the rule. This breaks the linear-time complexity guarantee from Maher (2001).
- **Priest's LP** interprets the third value as "both true and false" (paraconsistent). This is semantically inappropriate — during reasoning, an unknown literal is not contradictory, it is merely undetermined.
- **Strong Kleene** correctly models incomplete information: `AND(False, Unknown) = False` (if one conjunct is false, the conjunction is false regardless), while `AND(True, Unknown) = Unknown` (we cannot determine the conjunction). This matches the defeasible engine's existing behavior, where a single disproven body literal discards the entire rule.

**Consequences:**
- Fitting (1991) proves K3 gives correct fixpoint semantics for logic programs with NAF.
- The `rule_discarded` flag in the current engine is a special case of K3 conjunction: it tracks whether `AND(body₁, body₂, …, bodyₙ) = Negative`.

---

### ADR-002: Balanced Ternary i8 Over Enum With Discriminant

**Decision:** Represent `Trit` as `#[repr(i8)]` enum with values `-1`, `0`, `+1`.

**Context:** Two representations were considered:

1. **Plain enum** (`#[repr(u8)]` with discriminants 0, 1, 2): Requires lookup tables or match arms for arithmetic operations.
2. **Balanced ternary** (`#[repr(i8)]` with discriminants -1, 0, +1): Enables branch-free arithmetic — `not(a) = -a`, `and(a,b) = min(a,b)`, `or(a,b) = max(a,b)`.

**Trade-off analysis:**

The balanced ternary encoding maps directly to Łukasiewicz's original formulation and to the Kleene truth tables. Negation is literal integer negation. Conjunction is integer minimum. Disjunction is integer maximum. No lookup tables, no match arms, no branches — just arithmetic that the compiler can vectorize.

The trade-off is that `0` (default i8) means `Unknown` rather than a sentinel. This is the correct default: a freshly constructed literal whose polarity has not been set should be `Unknown`, not `Positive` or `Negative`.

**Consequences:**
- `Default for Trit` returns `Unknown` (zero-initialized).
- `From<bool>` maps `true → 1`, `false → -1` (no zero involved, so the conversion is total).
- Unsafe `transmute` from arbitrary `i8` is forbidden; only `-1`, `0`, `1` are valid.

---

### ADR-003: Two-Bit LitId Encoding Over Separate Unknown Set

**Decision:** Encode polarity as 2 bits within `LitId` rather than maintaining a separate `FxHashSet` for unknown literals.

**Context:** Two approaches:

1. **Separate set**: Keep `LitId` as-is (1-bit negation), add `unknown_literals: FxHashSet<LitId>` to `ReasoningState`.
2. **Inline encoding**: Use bits [31:30] for polarity, reducing atom capacity from 2³¹ to 2³⁰.

**Trade-off analysis:**

The separate set approach preserves binary compatibility but introduces a second lookup on every literal access during reasoning — every `is_definitely_proven()` call would also need to check `is_unknown()`. This is a hot path (called per rule per body literal per iteration).

The inline encoding maintains O(1) single-lookup semantics and keeps all polarity information co-located. The capacity reduction from 2³¹ to 2³⁰ atoms is academic — no practical defeasible theory approaches even 2²⁰ atoms.

**Consequences:**
- `LitId::complement()` becomes a 2-bit XOR (`00 ↔ 10`) rather than a 1-bit XOR.
- `LitId::new()` takes `Trit` instead of `bool`.
- Bit pattern `11` is reserved; constructing it is a debug assertion failure.

---

### ADR-004: Phased Migration With Deprecation Period

**Decision:** Implement as a two-phase migration to minimize disruption.

**Phase 1 — Introduce `Trit`, preserve `bool` API:**
- Add `Trit` type to `spindle-core`.
- Add `polarity: Trit` field to `Literal` (replacing `negation: bool` internally).
- Provide deprecated `is_negated()` → `polarity().is_negative()` bridge.
- All existing tests continue to pass via `From<bool>` conversion.

**Phase 2 — Propagate through reasoning engine:**
- Update `LitId` to 2-bit encoding.
- Update `LiteralBitSet` to 3-state tracking.
- Replace `rule_discarded: FxHashMap<&str, bool>` with `Trit`-valued map.
- Introduce `Unknown` into Phase 2 fixed-point loop.
- Remove deprecated `bool`-based APIs.

**Consequences:**
- Phase 1 is a purely additive change — no semantic changes to reasoning.
- Phase 2 changes reasoning internals but not observable output (for existing theories without `Unknown` literals, behavior is identical).

---

## 6. Contract Specifications

### CON-001: Trit Public API

```rust
/// Balanced ternary truth value following Strong Kleene (K3) logic.
///
/// Represents three truth states:
/// - `Positive` (+1): asserted / true
/// - `Unknown` (0): indeterminate / not yet evaluated
/// - `Negative` (-1): negated / false
#[repr(i8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Trit {
    Negative = -1,
    #[default]
    Unknown = 0,
    Positive = 1,
}

impl Trit {
    // --- Predicates ---
    pub fn is_positive(self) -> bool;
    pub fn is_negative(self) -> bool;
    pub fn is_unknown(self) -> bool;
    pub fn is_determined(self) -> bool;

    // --- Kleene operations (branch-free) ---
    /// Kleene negation: -a
    pub fn not(self) -> Trit;
    /// Kleene conjunction: min(a, b)
    pub fn and(self, other: Trit) -> Trit;
    /// Kleene disjunction: max(a, b)
    pub fn or(self, other: Trit) -> Trit;
    /// Kleene material implication: max(-a, b)
    pub fn implies(self, other: Trit) -> Trit;

    // --- Conversions ---
    pub fn to_bool(self) -> Option<bool>;
    pub fn to_bool_or(self, default: bool) -> bool;
    pub fn from_i8_clamped(v: i8) -> Trit;
}

impl From<bool> for Trit { /* true → Positive, false → Negative */ }
impl From<Trit> for i8 { /* Positive → 1, Unknown → 0, Negative → -1 */ }
impl TryFrom<i8> for Trit { /* -1, 0, 1 → Ok; others → Err */ }
impl std::ops::Not for Trit { type Output = Trit; }
impl std::ops::BitAnd for Trit { type Output = Trit; }
impl std::ops::BitOr for Trit { type Output = Trit; }
impl fmt::Display for Trit { /* "+", "?", "-" */ }
```

Pre-conditions:
- For `TryFrom<i8>`: input must be in `{-1, 0, 1}`.

Post-conditions:
- `Trit::not()`: `a.not().not() == a` (involution).
- `Trit::and()`: `a.and(b) == b.and(a)` (commutativity).
- `Trit::or()`: `a.or(b) == b.or(a)` (commutativity).
- `Trit::and()` and `Trit::or()`: De Morgan's laws hold: `(a.and(b)).not() == a.not().or(b.not())`.

Implements:
- REQ-001 through REQ-008

Verified by:
- TEST-001 through TEST-008

---

### CON-002: Updated Literal API

```rust
pub struct Literal {
    name_id: InternedLiteralName,
    pub polarity: Trit,           // was: pub negation: bool
    pub mode: Mode,
    pub temporal: Temporal,
    pub temporal_expr: Option<TemporalExpr>,
    predicate_args: Vec<Term>,
    pub interval_var: Option<SymbolId>,
}

impl Literal {
    pub fn simple(name: impl AsRef<str>) -> Self;    // polarity = Positive
    pub fn negated(name: impl AsRef<str>) -> Self;   // polarity = Negative
    pub fn unknown(name: impl AsRef<str>) -> Self;   // polarity = Unknown (NEW)

    pub fn polarity(&self) -> Trit;                  // NEW
    pub fn is_positive(&self) -> bool;               // NEW
    pub fn is_unknown(&self) -> bool;                // NEW

    #[deprecated(note = "use polarity().is_negative()")]
    pub fn is_negated(&self) -> bool;

    pub fn complement(&self) -> Self;                // uses Trit::not()
}
```

Pre-conditions: None.

Post-conditions:
- `Literal::simple(n).polarity() == Trit::Positive`.
- `Literal::negated(n).polarity() == Trit::Negative`.
- `Literal::unknown(n).polarity() == Trit::Unknown`.
- `Literal::complement()` flips `Positive` ↔ `Negative`, preserves `Unknown`.

Implements:
- REQ-009, REQ-010

Verified by:
- TEST-009, TEST-010

---

### CON-003: Updated LitId API

```rust
pub struct LitId(u32);

impl LitId {
    const POLARITY_SHIFT: u32 = 30;
    const POLARITY_MASK: u32 = 0b11 << 30;
    const ATOM_MASK: u32 = !Self::POLARITY_MASK;

    pub fn new(atom: AtomId, polarity: Trit) -> Self;
    pub fn polarity(self) -> Trit;
    pub fn atom(self) -> AtomId;
    pub fn complement(self) -> Self;

    #[deprecated(note = "use polarity().is_negative()")]
    pub fn is_negated(self) -> bool;
}
```

Pre-conditions:
- `atom.as_raw() < 2^30` (enforced by debug assertion).
- `polarity != Unknown` for complement (debug assertion; complement of Unknown is Unknown).

Post-conditions:
- `LitId::new(a, p).atom() == a`.
- `LitId::new(a, p).polarity() == p`.
- `LitId::new(a, Positive).complement().polarity() == Negative`.
- `LitId::new(a, Negative).complement().polarity() == Positive`.
- `LitId::new(a, Unknown).complement().polarity() == Unknown`.

Implements:
- REQ-011

Verified by:
- TEST-011

---

### CON-004: Updated LiteralBitSet API

```rust
pub(crate) struct LiteralBitSet {
    bits: FixedBitSet,  // 3 bits per atom: [positive, unknown, negated]
}

impl LiteralBitSet {
    pub(crate) fn new(atom_count: usize) -> Self;
    pub(crate) fn contains(&self, id: LitId) -> bool;
    pub(crate) fn insert(&mut self, id: LitId);
    pub(crate) fn contains_any_polarity(&self, atom: AtomId) -> bool;  // NEW
    pub(crate) fn polarity_of(&self, atom: AtomId) -> Option<Trit>;    // NEW
}
```

Pre-conditions: None.

Post-conditions:
- After `insert(id)`, `contains(id)` returns `true`.
- Monotonicity: once `contains(id)` returns `true`, it remains `true`.
- `contains_any_polarity(a)` returns `true` iff `contains(LitId::new(a, Positive)) || contains(LitId::new(a, Unknown)) || contains(LitId::new(a, Negative))`.

Implements:
- REQ-012

Verified by:
- TEST-012

---

### CON-005: Updated ReasoningState API

```rust
pub(crate) struct ReasoningState<'a> {
    pub(crate) worklist: VecDeque<Literal>,
    pub(crate) enqueued: LiteralBitSet,
    pub(crate) definite_proven: LiteralBitSet,
    pub(crate) defeasible_proven: LiteralBitSet,
    pub(crate) defeasible_disproven: LiteralBitSet,
    pub(crate) definite_body_remaining: FxHashMap<&'a str, usize>,
    pub(crate) defeasible_body_remaining: FxHashMap<&'a str, usize>,
    pub(crate) rule_body_status: FxHashMap<&'a str, Trit>,  // was: rule_discarded: bool
    pub(crate) conclusions: Vec<Conclusion>,
}
```

Pre-conditions: None.

Post-conditions:
- `rule_body_status` initialized to `Trit::Unknown` for all rules at Phase 2 start.
- Transitions: `Unknown → Negative` (body literal disproven), `Unknown → Positive` (all body literals proven). No backward transitions.
- Conclusion output is identical to the current engine for all theories without `Unknown`-polarity input literals.

Implements:
- REQ-013

Verified by:
- TEST-013

---

## 7. Purity Boundary Map

### Pure Core (no I/O, no shared state, deterministic)

- `Trit::not()`, `Trit::and()`, `Trit::or()`, `Trit::implies()`: Balanced ternary arithmetic.
- `Trit::to_bool()`, `Trit::from_i8_clamped()`: Value conversion.
- `LitId::new()`, `LitId::polarity()`, `LitId::complement()`: Bit manipulation.
- `LiteralBitSet::contains()`, `LiteralBitSet::insert()`: Bitset operations.

### Effectful Shell (orchestrates I/O, calls pure core)

- `ReasoningState` mutation during forward chaining.
- Serde serialization/deserialization of `Trit` and `Literal`.

### Boundary Contracts (data types crossing the boundary)

- `Trit`: flows from pure core into effectful shell (reasoning engine) and out to serialization.
- `LitId`: flows between index construction (effectful — builds from theory) and bitset operations (pure).

### Dependency Rule

Dependencies point inward: reasoning engine → `Trit`/`LitId`/`LiteralBitSet`. Pure core MUST NOT import from reasoning engine.

### Enforcement

Module visibility (`pub(crate)` on reasoning internals) and Rust's type system. `Trit` is a public type in `spindle-core`.

---

## 8. Test Specifications

### TEST-001: Trit Construction and Defaults

Verify that `Trit` can be constructed in all three states, that `Default` produces `Unknown`, and that `size_of::<Trit>() == 1`.

Trace: REQ-001, NFR-001

---

### TEST-002: Kleene Negation Exhaustive

Property-based: for all `t: Trit`, `t.not().not() == t` (involution). Exhaustive: verify the 3-row truth table.

Trace: REQ-002

---

### TEST-003: Kleene Conjunction Exhaustive

Exhaustive: verify all 9 cells of the conjunction truth table. Property-based: commutativity (`a.and(b) == b.and(a)`), associativity, identity element (`Positive`), annihilator (`Negative`).

Trace: REQ-003

---

### TEST-004: Kleene Disjunction Exhaustive

Exhaustive: verify all 9 cells of the disjunction truth table. Property-based: commutativity, associativity, identity element (`Negative`), annihilator (`Positive`).

Trace: REQ-004

---

### TEST-005: Kleene Implication Exhaustive

Exhaustive: verify all 9 cells. Property: `a.implies(b) == a.not().or(b)`.

Trace: REQ-005

---

### TEST-006: Trit Predicates

For each variant, verify exactly one of `is_positive()`, `is_negative()`, `is_unknown()` returns `true`. Verify `is_determined()` returns `true` for `Positive` and `Negative`, `false` for `Unknown`.

Trace: REQ-006

---

### TEST-007: Bool Roundtrip

Property: for all `b: bool`, `Trit::from(b).to_bool() == Some(b)`. Verify `Unknown.to_bool() == None`. Verify `Unknown.to_bool_or(true) == true`.

Trace: REQ-007

---

### TEST-008: i8 Roundtrip and Clamping

Property: for `v ∈ {-1, 0, 1}`, `i8::from(Trit::try_from(v).unwrap()) == v`. Verify `Trit::try_from(2i8).is_err()`. Verify `Trit::from_i8_clamped(42) == Positive`, `Trit::from_i8_clamped(-42) == Negative`.

Trace: REQ-008

---

### TEST-009: Literal Polarity

Verify `Literal::simple("p").polarity() == Positive`. Verify `Literal::negated("p").polarity() == Negative`. Verify `Literal::unknown("p").polarity() == Unknown`. Verify complement of each.

Trace: REQ-009

---

### TEST-010: LiteralStruct Serde

Verify serialization/deserialization of `LiteralStruct` with all three polarity values. Verify backward-compatible deserialization from `{"negated": true}` schema.

Trace: REQ-010, REQ-015

---

### TEST-011: LitId Ternary Encoding

Verify `LitId::new(atom, polarity).polarity()` roundtrips for all three polarities. Verify complement. Verify atom recovery. Verify debug assertion on bit pattern `11`.

Trace: REQ-011

---

### TEST-012: LiteralBitSet Three-State

Verify insert and contains for all three polarities. Verify `contains_any_polarity`. Verify monotonicity (no remove). Verify auto-grow.

Trace: REQ-012

---

### TEST-013: Reasoning Equivalence

Run the full existing test suite of SPL theories through the refactored engine. Verify that all conclusions are byte-identical to the current engine's output.

Trace: REQ-013, NFR-003

---

### TEST-014: Display Formatting

Verify `Trit` display: `"+"`, `"?"`, `"-"`. Verify literal display with `?` prefix for unknown polarity.

Trace: REQ-014

---

### TEST-015: Backward-Compatible Deserialization

Verify that JSON `{"negation": true, ...}` deserializes to `Literal` with `polarity: Negative`. Verify `{"negation": false, ...}` → `Positive`.

Trace: REQ-015

---

### TEST-016: Memory Size Assertions

`static_assert` (compile-time or runtime) that `size_of::<Trit>() == 1` and `size_of::<Literal>()` has not increased.

Trace: NFR-001

---

### TEST-017: Branch-Free Operations (Benchmark)

Benchmark `Trit` operations against equivalent `match`-based implementations. Verify no performance regression.

Trace: NFR-002

---

### TEST-018: Reasoning Performance Regression

Run the existing benchmark suite before and after refactoring. Verify wall-clock time does not regress by more than 5%.

Trace: NFR-003

---

## 9. Verification Strategy

| System Characteristic | Technique | Scope |
|---|---|---|
| `Trit` arithmetic (pure, algebraic) | Property-based testing + exhaustive (only 3³ = 27 cases for binary ops) | TEST-002 through TEST-005 |
| Algebraic laws (De Morgan, involution, commutativity) | Property-based testing | TEST-002 through TEST-005 |
| `LitId` bit manipulation (pure) | Exhaustive over polarity × sample atoms | TEST-011 |
| `LiteralBitSet` monotonicity | Property-based (insert-then-contains never reverts) | TEST-012 |
| Serialization roundtrip | Property-based (serialize-deserialize identity) | TEST-010, TEST-015 |
| Reasoning equivalence | Integration (full theory execution, diff output) | TEST-013 |
| Performance regression | Benchmark comparison | TEST-017, TEST-018 |
| Mutation testing | Post-implementation on `Trit` module and `LitId` encoding | Kill rate ≥ 95% |

---

## 10. Migration Plan

### Phase 1: Introduce Trit (Additive, Non-Breaking)

1. Create `trit.rs` in `spindle-core` with the `Trit` type and all operations (REQ-001 through REQ-008).
2. Add `polarity: Trit` to `Literal`, replacing `negation: bool` internally but providing deprecated `is_negated()` bridge (REQ-009).
3. Update `LiteralStruct` (REQ-010).
4. Update all call sites in `spindle-core`, `spindle-parser`, `spindle-cli`, `spindle-wasm` to use `Trit::from(bool)` where needed.
5. All existing tests pass via the `From<bool>` bridge.

### Phase 2: Propagate Through Engine (Internal Change)

1. Update `LitId` to 2-bit encoding (REQ-011).
2. Update `LiteralBitSet` to 3-state tracking (REQ-012).
3. Replace `rule_discarded` with `rule_body_status: Trit` (REQ-013).
4. Run full test suite + benchmark comparison (TEST-013, TEST-018).
5. Remove deprecated `bool`-based APIs.

### Phase 3: Extension Interface (Future Spec)

1. Expose `Unknown` polarity through the extension interface for external consumers.
2. Add `Unknown`-polarity literals to SPL syntax (optional; separate spec).

---

## 11. Observability

### OBS-001: Trit Distribution in Conclusions

Log or expose a metric counting conclusions by polarity (`Positive`, `Unknown`, `Negative`) × proof tag (`+D`, `-D`, `+d`, `-d`) at the end of each reasoning pass. This enables monitoring whether `Unknown` propagation is functioning correctly — in a well-formed theory with complete facts, `Unknown` counts should be zero at termination.

Trace: REQ-013

---

## 12. Risks and Mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| `Unknown` leaks into conclusions for theories that should be fully determined | S2 | TEST-013 verifies byte-identical output for existing theories; OBS-001 monitors Unknown counts |
| `LitId` capacity reduction breaks large theories | S4 | 2³⁰ atoms is 3 orders of magnitude beyond practical use; add a compile-time assertion |
| `From<bool>` bridge masks bugs during migration | S3 | Phase 2 removes deprecated APIs; mutation testing verifies coverage |
| Branch-free arithmetic breaks on non-two's-complement platforms | S4 | Rust guarantees two's-complement for `i8`; add `static_assert` |
| Serde backward compatibility breaks existing serialized theories | S2 | TEST-015 covers both old and new schemas; custom deserializer with `#[serde(alias)]` |
