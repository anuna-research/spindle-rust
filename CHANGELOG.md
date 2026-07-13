# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows pre-1.0 Semantic Versioning (`0.y.z`).

## [Unreleased]

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
- **Breaking**: bounded temporal queries now match exact windows (SPEC-020
  REQ-006). `query`, `requires`, `what_if`, and `abduce` goals carrying a
  bounded temporal window (e.g. `p@[1,10]`) only match conclusions with the
  *identical* window. Previously the window was ignored, so a bounded query
  matched any conclusion in the same family — including atemporal `p` or
  `p@[20,30]`. A query window strictly contained in a proven window (query
  `p@[1,10]` vs proven `p@[0,20]`) now also returns `unknown`. This applies
  to the CLI (`spindle query`/`requires`/`what-if`) and WASM surfaces even
  though the JSON envelope schemas (`spindle.query.v1`, `spindle.requires.v2`)
  are unchanged — only the reported status for bounded queries differs.
  Atemporal queries still match any family member. Call
  `query_with_match_mode(theory, literal, QueryMatchMode::Family)` to restore
  family-wide matching for a bounded literal.
- **Breaking**: `AbductionSolution.facts` changed from `HashSet<Literal>` to
  `Vec<Literal>`, deduplicated by injective canonical key so distinct temporal
  windows and typed terms are no longer collapsed.
- **Breaking**: `AbductionSolution.rules_used` now lists only the rules that
  produce that specific solution's fact-set, not every rule whose head matches
  the goal.
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
