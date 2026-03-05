# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows pre-1.0 Semantic Versioning (`0.y.z`).

## [0.4.0]

### Added
- **Extension function mechanism** (Phase 1+2): pluggable `FunctionRegistry` with
  `ExtensionFunction` trait. All built-in arithmetic operators (`+`, `-`, `*`, `/`,
  `div`, `rem`, `**`, `abs`, `min`, `max`, `round`, `floor`, `ceil`) migrated from
  hard-coded dispatch to registered extension functions.
- **Fold aggregation**: `(fold ?result identity reducer extract pattern)` construct
  for aggregating values across matching facts. Supports `required` identity for
  aggregates where an empty set is meaningless. Works with all numeric and temporal types.
- **Stratified reasoning**: automatic stratification for programs containing fold.
  Fold patterns are evaluated bottom-up by stratum, ensuring stable aggregation
  results. Temporal features cannot be combined with multi-stratum fold programs.
- **Date/time types**: five new `Term` variants — `Date`, `Time`, `Datetime`,
  `Duration`, `Offset` — with literal syntax (`#d:`, `#t:`, `#dt:`, `#dur:`, `#off:`).
- **Temporal functions**: 16 extension functions for date/time operations:
  - Construction: `datetime`
  - Extraction: `date-of`, `time-of`, `day-of-week`, `year-of`, `month-of`, `day-of-month`
  - Difference: `hours-between`, `minutes-between`, `days-between`, `duration-hours`, `duration-minutes`
  - Calendar arithmetic: `add-months`, `add-years`, `months-between`, `years-between`
- **Temporal operator overloads**: `+`, `-`, `*`, `/`, `min`, `max` extended to
  work with temporal types (e.g., `Datetime + Duration → Datetime`,
  `Date - Date → Integer`, `Duration * Number → Duration`).
- **Temporal comparisons**: `<`, `>`, `<=`, `>=` work with same-type temporal values.
  Datetime comparisons are by instant (UTC-equivalent), enabling cross-timezone ordering.

### Changed
- Version bumped to `0.4.0` (minor bump for new types and functions under pre-1.0 semver).
- `Term` `PartialEq`, `Eq`, and `Hash` are now manually implemented to support
  `DateTime<FixedOffset>` (compares by UTC instant, hashes by timestamp).

### Fixed
- Datetime `Hash`/`Eq` consistency: values representing the same instant at different
  offsets now hash identically and compare as equal.
- Runtime arity guards added to all built-in `eval()` methods.
- CLI and WASM stratification bypass resolved.
- Fold/stratification validation and bug fixes.

## [0.3.0]

### Added
- **Arithmetic module** (SPEC-017): full arithmetic expression support in SPL.
  - `Term` enum with `Symbol`, `Integer`, `Decimal`, `Float` variants.
  - `FiniteFloat` wrapper: rejects NaN/Inf, normalizes `-0.0`, safe for `Eq`/`Hash`.
  - `ArithExpr` AST with `NaryOp` (`+`, `-`, `*`, `/`, `min`, `max`),
    `BinOp` (`div`, `rem`, `**`), and `UnaryOp` (`abs`).
  - `ArithConstraint`: `bind` variable binding and comparison guards
    (`=`, `!=`, `<`, `>`, `<=`, `>=`).
  - `BodyLiteral`, `BodyLogicLiteral`, and `BodyArg` types for mixed
    logic/arithmetic rule bodies.
  - Cross-type numeric matching in grounding (REQ-010/CON-005):
    `Integer(2)` matches `Decimal(2.0)` matches `Float(2.0)`.
  - Type promotion chain: Integer -> Decimal -> Float.
  - `rust_decimal` dependency for arbitrary-precision decimal arithmetic.
- **SPL parser extensions**:
  - Arithmetic expression parser for `+`, `-`, `*`, `/`, `div`, `rem`,
    `**`, `abs`, `min`, `max`.
  - `(bind ?var expr)` and comparison guard parsing in rule bodies.
  - Arithmetic expressions in body literal argument positions.
  - Numeric literal detection in predicate arguments.
  - Lexer extended to accept operator characters in atoms.
  - Parse-time guard checks: reserved keyword rejection (REQ-008),
    arithmetic in head rejection (REQ-009), negated arithmetic
    rejection (REQ-011).
- **v2 JSON output** (REQ-012/CON-006):
  - `--v2` flag on CLI, `reasonV2` method on WASM.
  - Typed `Term` arguments in JSON schema (`spindle.reason.v2`).
- **Test suites**:
  - Unit tests for `Term`, `ArithExpr`, and type promotion (TEST-001, TEST-002, TEST-005).
  - Arithmetic parsing and guard enforcement integration tests.
  - Grounding integration tests with arithmetic pipeline.
  - Worked examples, NFR, and proptest suites for arithmetic.
  - v2 JSON typed argument serialization tests (TEST-012).

### Changed
- **Breaking**: `From<NumericValue> for Term` replaced with `TryFrom<NumericValue> for Term`.
  Non-finite floats (NaN, Inf) now return an error instead of silently coercing to `0.0`.
- `Literal::predicate_ids` migrated from `Vec<SymbolId>` to `Vec<Term>`.
- `Substitution::terms` migrated from `SymbolId` values to `Term` values.
- `RuleBody` migrated from `SmallVec<[Literal; 4]>` to `SmallVec<[BodyLiteral; 4]>`.
- Body literals evaluated in source order with threaded substitutions.
- Temporal variables rejected as arithmetic operands (REQ-006).

### Fixed
- `FiniteFloat` serde deserialization now validates through `FiniteFloat::new`,
  preventing non-canonical values (`-0.0`) and non-finite values from bypassing
  type invariants.
- `BinArithOp::Pow` Display emits `**` (matching the SPL parser) instead of `pow`.
- `BodyLogicLiteral::to_spl()` renders `BodyArg::Arith` directly instead of
  quoting through `render_spl_atom`, preserving arithmetic s-expression syntax
  on round-trip.
- Tilde-negated reserved keywords (`~>`, `~bind`, etc.) rejected in list-form
  literals in both body and head parsers (REQ-008).
- Negative base with fractional exponent rejected in `decimal_pow`.
- Bind consistency enforced; constant arithmetic args grounded correctly;
  numeric parsing unified across paths.
- Temporal bounds preserved in body normalization.
- Non-finite floats rejected in bind evaluation.
- Arithmetic module memory leaks resolved; duplicate fact double-decrement fixed.

## [0.2.0]

### Added
- Verified `requires` core API:
  - `requires_with_options(theory, goal, options)`
  - `RequiresOptions`, `RequiresResult`, `RequiresSearchStatus`, `RequiresVerificationStats`
- New CLI contract schema: `spindle.requires.v2`.
- New core test suite: `crates/spindle-core/tests/requires_verified_tests.rs`.

### Changed
- `requires` is now verified-by-default in core and CLI.
- `requires --json` emits `spindle.requires.v2` only.
- `spindle capabilities --json` now advertises `schemas.requires = spindle.requires.v2`.
- Core `requires()` compatibility wrapper now delegates to verified logic.

### Fixed
- Eliminated false-positive `requires` candidates that fail under full defeasible reasoning.
- Corrected `BudgetExhausted` classification for duplicate raw-candidate edge cases.
- Added defensive collision handling for injected verification fact labels.

### Migration
- Clients validating `requires` JSON should migrate from `spindle.requires.v1` to `spindle.requires.v2`.
- In v2, `satisfied=false` with `solutions=[]` is valid.
