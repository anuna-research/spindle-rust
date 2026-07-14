# Changelog

All notable changes to Spindle-Rust are documented here.
For the full changelog with migration notes, see [CHANGELOG.md](https://codeberg.org/anuna/spindle-rust/src/branch/main/CHANGELOG.md) in the repository root.

## Unreleased

### Added

- **SPEC-024 Predicate Model & Vocabulary**: a structural predicate identity and
  derived tooling projections, all additive and non-semantic.
  - First-class SPL `(predicate name ((arg sort) ...))` declarations and
    structured `(meta (predicate functor arity) ...)` metadata targets.
    See the [SPL Format reference](../reference/spl.md#predicate-declarations).
  - `PredicateSymbol`, `PredicateSignature`, `ArgumentProfile`, `TheorySignature`,
    `Vocabulary`, `Shape`, and `GroundLiteral`/`LiteralPattern` phase wrappers.
    See the [Rust Library guide](../integration/rust.md#predicate-vocabulary).
  - `Theory::metadata()` now returns `&HashMap<MetaTarget, Meta>` (internal
    breaking change); `add_meta`/`get_meta` remain label wrappers.

## v0.3.0 (2026-03-04)

### Added

- **SPEC-017 Arithmetic Module**: full arithmetic expression support in rule bodies.
  - Three numeric term types: `Integer`, `Decimal` (arbitrary-precision via `rust_decimal`), `Float` (`FiniteFloat` wrapper rejecting NaN/Inf).
  - Automatic type promotion chain: Integer -> Decimal -> Float.
  - `ArithExpr` AST with n-ary ops (`+`, `-`, `*`, `/`, `min`, `max`), binary ops (`div`, `rem`, `**`), and unary ops (`abs`).
  - `bind` variable binding and comparison guards (`=`, `!=`, `<`, `>`, `<=`, `>=`) in rule bodies.
  - Cross-type numeric matching in grounding (e.g., `Integer(2)` matches `Decimal(2.0)`).
  - SPL parser extended with arithmetic expression, numeric literal, and guard parsing.
  - Parse-time rejection of arithmetic in heads, negated arithmetic, and reserved keywords.
- **SPEC-018 Temporal Atom Identity**: atoms with different temporal bounds are now treated as distinct during reasoning.
  - Synthetic bridging rules connect temporally-scoped variants.
  - Core query operators (`what_if`, `why_not`, `requires`) upgraded to temporal-aware matching.
- **v2 JSON output** (REQ-012): `--v2` CLI flag and `reasonV2` WASM method emitting typed `Term` arguments in schema `spindle.reason.v2`.

### Changed

- **Breaking**: `From<NumericValue> for Term` replaced with `TryFrom`; non-finite floats now return errors.
- `Literal::predicate_ids` and `Substitution::terms` migrated from `SymbolId` to `Term`.
- `RuleBody` migrated from `SmallVec<[Literal; 4]>` to `SmallVec<[BodyLiteral; 4]>`.

### Fixed

- `FiniteFloat` serde deserialization validates invariants, preventing non-canonical or non-finite values.
- SPL round-trip for arithmetic expressions preserved correctly.
- Tilde-negated reserved keywords (`~>`, `~bind`, etc.) rejected in list-form literals.
- Negative base with fractional exponent rejected in `decimal_pow`.
- Temporal bounds preserved in body normalization.

## v0.2.0 (2026-02-26)

### Added

- **IMPL-011 Verified `requires`**: `requires` command now verifies abduction solutions by default.
  - New core API: `requires_with_options(theory, goal, options)` with `RequiresOptions`, `RequiresResult`, `RequiresSearchStatus`, `RequiresVerificationStats`.
  - New CLI contract schema: `spindle.requires.v2`.
- **Release tooling**: `release.sh` script with pre-flight validation checks and `make release` target.
- Pre-1.0 semver policy documented.

### Changed

- `requires --json` emits `spindle.requires.v2` schema only.
- `spindle capabilities --json` advertises `schemas.requires = spindle.requires.v2`.
- Core `requires()` compatibility wrapper delegates to verified logic.

### Fixed

- Eliminated false-positive `requires` candidates that fail under full defeasible reasoning.
- Corrected `BudgetExhausted` classification for duplicate raw-candidate edge cases.
- Defensive collision handling for injected verification fact labels.
