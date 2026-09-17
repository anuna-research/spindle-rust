# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows pre-1.0 Semantic Versioning (`0.y.z`).

## [Unreleased]

### Added
- **Aggregation in SPL**: named `agg` premises (`sum`, `count`, `min-of`,
  `max-of`) and explicit `(bind ?result (fold ...))` expressions.
  - Aggregate over completed, positively proved predicates, including derived
    rows, with grouping and inferred strata. Cycles through aggregates are rejected.
  - Distinct rows contribute separately even when their selected values match;
    multiple proofs of the same row contribute once.
  - Results bind directly without an enumerated output domain. `sum` and `count`
    return zero on empty input; `min-of` and `max-of` fail the premise.
  - Snapshot premises preserve defeasible evidence, source labels, and priorities;
    internal snapshot predicates are hidden from conclusions.
  - Checked integer arithmetic and bounded grounding report overflow or exhausted
    budgets as errors. The aggregate bridge currently excludes decimal/float,
    modal, temporal, and trust-weighted inputs.
  - Syntax, embedding guidance, and a runnable example in
    [Aggregation and extension functions](https://git.anuna.io/anuna-research/spindle-rust/src/branch/main/docs/aggregation-extensions.md).
- **Extension functions**: `ExtensionFunction`, `FunctionRegistry`, and
  `PrepareOptions::function_registry` support host-registered pure functions,
  integer/symbol value bindings, and custom named aggregators. The builtin
  prelude includes arithmetic, `round` (half-to-even), `floor`, and `ceil`.
- **Trust diminishment**: applicable but overruled defeaters reduce defeasible
  conclusion credibility multiplicatively using their weakest-link degrees.
  Thresholds use the diminished degree, `WeightedConclusion::diminished_by`
  records the challenges, and definite conclusions are exempt.
- **Lean verification and differential testing**: models and proofs for
  grounding, arithmetic, temporal intervals, query operators, and trust.
  Aggregate coverage includes dependency/stratum inference, source semantics, lowering,
  and completed-prefix equivalence for all four proof tags. Rust/Lean oracle suites
  compare supported fragments; these do not establish whole-language Rust
  conformance or verify custom extension implementations.
- **Verification gate**: builds Lean libraries and oracle executables, rejects
  admitted proofs and local axioms, checks for vacuous proofs, and audits theorem
  dependencies.
- **Aggregate benchmarks**: `make bench-aggregation` measures preparation and
  full reasoning across row counts, groups, unrelated facts, and chained stages,
  with a documented local baseline.
- **Predicate model and vocabulary** (SPEC-024): a structural predicate identity
  and derived tooling projections, all additive and non-semantic (reasoning is
  unchanged).
  - `PredicateSymbol` (functor + arity) with a `HasPredicateSymbol` projection
    for `Literal` and `BodyLogicLiteral` (body arity retains arithmetic args).
  - Primitive sorts, checked `PredicateSignature`, positional `ArgumentProfile`,
    and non-semantic `Shape` validation.
  - `GroundLiteral` / `LiteralPattern` phase wrappers and `Literal::classify`.
  - `TheorySignature::derive` and `Vocabulary::derive` (deterministic symbol
    sets, declaration conflict handling, descriptions, provenance, summary
    counts).
  - SPL: first-class `(predicate name ((arg sort) ...))` declarations and
    structured `(meta (predicate functor arity) ...)` metadata targets, stored
    in `Theory` with source provenance. Undeclared predicates remain valid.
    Declarations also accept inline `meta` properties
    (`(predicate name (...) (description "..."))`) as sugar for the separate
    metadata target.
  - Predicate-indicator recognizer (`functor/arity`) in `spindle-parser`.
  - Additive `spindle.vocabulary/1` JSON DTOs in `spindle-contract`.
  - `Theory::metadata()` retains its label-keyed API; predicate metadata is
    available separately through `Theory::predicate_metadata()`.
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
  - `rust_decimal` dependency for fixed-precision decimal arithmetic.
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
- **Reasoning semantics**: Rust and the standard Lean oracles now follow
  traditional ambiguity-blocking DL(∂) with four constructive proof tags.
  Unsupported cycles remain undecided instead of receiving automatic negative
  conclusions; undecided attackers are not discarded. Contradictory definite
  facts retain both positive definite and defeasible tags without proving
  unrelated literals.
- **Breaking**: arithmetic expressions use registry-dispatched `ArithExpr::Call`
  nodes, replacing the operator-specific AST variants, with `Value` and `Fold`
  variants for general bindings and aggregation.
- Projection snapshots and abduction output use deterministic, injective
  semantic keys, preserving distinct temporal windows and typed terms.
- CI workflows restored for Forgejo compatibility.
- Documentation book uses a forge-neutral repository icon and canonical repository
  source links in place of edit links ([#37](https://codeberg.org/anuna/spindle-rust/pulls/37),
  contributed by SamB).
- Refreshed the documentation book with aggregation and verification guides,
  current DL(∂) semantics, query and WASM APIs, trust diminishment, and aggregate
  performance guidance. The book includes this changelog directly.
- **Breaking**: bounded temporal queries now match exact windows (SPEC-020
  REQ-006). `query`, `requires`, `what_if`, and `abduce` goals carrying a
  bounded temporal window (e.g. `p@[1,10]`) only match conclusions with the
  *identical* window. Previously the window was ignored, so a bounded query
  matched any conclusion in the same family — including atemporal `p` or
  `p@[20,30]`. A query window strictly contained in a proven window (query
  `p@[1,10]` vs proven `p@[0,20]`) now also returns `unknown`. This applies to CLI queries (`spindle query`/`requires`) and WASM query methods.
  The JSON envelope schemas (`spindle.query.v1`, `spindle.requires.v2`)
  are unchanged; only the reported status for bounded queries differs.
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
- Documentation examples now use complementary defeater heads and defeasible
  defaults correctly. Temporal documentation describes current interval variables,
  Allen constraints, and family matching instead of the removed bridge stage.
- Explanations resolve grounded rule labels to source templates, and `why_not`
  inspects grounded rules so variable-headed rules report actual blockers.
  Superiority checks use template labels, and defeated rules are no longer
  projected as supporting proofs.
- `what_if` deduplicates new conclusions proven at both positive tags while
  preserving distinct typed arguments and temporal windows.
- Predicate declarations and metadata survive grounding, wildcard rewriting,
  and temporal filtering. Vocabulary reports retain conflicting declarations
  and their provenance; deferred shape checks no longer count as mismatches.
- Vocabulary DTO validation rejects malformed functors and inconsistent
  diagnostics while accepting coherent v1 signatures that omit declaration origins. SPL rejects malformed metadata properties, and WASM
  output quotes structured predicate metadata targets correctly.
- Symbol-valued extension returns bind correctly, and float-to-integer boundary
  checks reject out-of-range values.
- Reasoning tracks repeated body occurrences per slot and discards attackers
  with disproved premises consistently.
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
- Clients checking `requires` JSON need the `spindle.requires.v2` schema instead of `spindle.requires.v1`.
- In v2, `satisfied=false` with `solutions=[]` is valid.
