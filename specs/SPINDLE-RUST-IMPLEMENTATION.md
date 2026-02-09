# Spindle-Rust Implementation Plan for gleg Contract

## 1. Purpose and Audience

This document is for spindle-rust maintainers.  
It defines the spindle-side implementation work needed to satisfy `specs/SPINDLE-CONTRACT.md`.

This is a delivery plan, not the wire contract.

## 2. Source of Truth

When in doubt:

1. Wire behavior and schema semantics: `specs/SPINDLE-CONTRACT.md` wins.
2. This file defines implementation sequencing and acceptance criteria.
3. `specs/spindle-rust-fixes-spec.md` is a deeper correctness RFC and technical appendix.

## 3. Current Gap Summary (Informative)

Known integration gaps to close:

1. Command JSON shapes are inconsistent.
2. `requires` contract lacks explicit `satisfied` behavior in current adapter expectations.
3. Stdin/givens/trust input flags need consistent support.
4. Capabilities endpoint must be complete and stable.
5. Limit/truncation semantics need explicit output fields/diagnostics.
6. `--at` capability must be either implemented end-to-end or advertised as unsupported.

## 4. Phased Implementation Plan

Phases may be developed in parallel, but Phase 1 acceptance criteria must be satisfied before Phase 2/3 changes are considered release-ready.

## Phase 1: Contract Baseline

Deliverables:

1. Unify JSON envelope across commands:
   - `schema_version`
   - `diagnostics`
2. Standardize literal fields:
   - `literal_spl`
   - `literal_struct`
3. Standardize status enum:
   - `provable|refuted|unknown`
4. Implement explicit `requires.satisfied` semantics.
5. Enforce status/trust separation:
   - trust never changes logical status.

Acceptance:

1. Contract tests pass against all command outputs.
2. Outputs validate against corresponding schemas:
   - `contracts/spindle/v1/schemas/spindle.reason.v1.schema.json`
   - `contracts/spindle/v1/schemas/spindle.query.v1.schema.json`
   - `contracts/spindle/v1/schemas/spindle.requires.v1.schema.json`
   - `contracts/spindle/v1/schemas/spindle.explain.v1.schema.json`
   - `contracts/spindle/v1/schemas/spindle.why_not.v1.schema.json`
   - `contracts/spindle/v1/schemas/spindle.capabilities.v1.schema.json`
3. `query unknown` returns exit code `0`.
4. No command emits legacy status synonyms such as `proven`, `true`, `disproven`.

## Phase 2: Stateless Input Ergonomics and Negotiation

Deliverables:

1. Theory input parity:
   - `reason [FILE]` or `--stdin`.
   - `query|requires|explain|why-not <LITERAL> [FILE]` or `--stdin`.
   - enforce exactly one theory source (`FILE` xor `--stdin`).
2. Givens flags:
   - `--given`
   - `--givens-file`
3. Trust flags:
   - `--source-weights-file`
   - `--trust-policy-file`
   - `--trust-mode`
4. Key-based trust merge conflict validation.
5. `capabilities --json` with stable `features`/`schemas`.
6. `--at` support for `query`, `requires`, `explain`, and `why-not` (or truthful `at=false` when unavailable).

Acceptance:

1. Input order does not affect reasoning/trust results.
2. Conflicting trust duplicate keys fail with validation diagnostics.

## Phase 3: Operational Limits and Optional RPC

Deliverables:

1. Enforce limits:
   - `--timeout-ms`
   - `--max-solutions`
   - `--max-ground-instances`
   - `--max-input-bytes`
   - `--max-trust-contributors`
2. Add truncation metadata for solution truncation and warning diagnostics:
   - `SOLUTIONS_LIMIT_HIT`
   - `truncated.solutions=true`
3. Optional:
   - `spindle run --json` stateless RPC-style wrapper reusing the same schemas.

Acceptance:

1. Truncation is machine-detectable.
2. Limits emit stable diagnostics and documented exit codes.

## 5. Temporal (`--at`) Contract Requirement

`capabilities.features.at` must be truthful.

Rules:

1. If `at=true`, `--at` must influence evaluation semantics for supported commands.
2. If not implemented, advertise `at=false` and return a clear validation diagnostic when provided.
3. This requirement is delivered in Phase 2.

## 6. Detailed Testing Strategy (Normative)

This section defines the mandatory testing approach for all spindle-rust changes.
We follow a TDD workflow: write failing contract tests first, then implement the fixes.

### 6.1 Rust-Side Contract Conformance Tests

Create `crates/spindle-cli/tests/contract_tests.rs`. This file is the primary gate for Phase 1.

**A. Schema Field Verification (Per Command)**

For each command (`reason`, `query`, `requires`, `explain`, `why-not`), implement tests that assert:

1.  **Envelope integrity**: `schema_version` is correct, `diagnostics` array exists.
2.  **Required fields**: All fields marked required in `specs/SPINDLE-CONTRACT.md` are present.
3.  **Field shapes**:
    *   `literal_struct` matches `{ functor, args, negated, mode, temporal }`.
    *   `grounding` matches `{ performed, had_variables, instances, limit_hit }`.
4.  **Data mapping**:
    *   `status` is exactly `provable|refuted|unknown` (no legacy `proven`/`true`).
    *   `conclusion_type` is `+D|+d|-D|-d` or null.
    *   `trust` is `null` when no trust input is provided.

**B. JSON Schema Validation (High ROI)**

Use the `jsonschema` crate to validate CLI output against the official gleg schemas.
This mechanically enforces the contract.

*   **Setup**: Copy or symlink `gleg/contracts/spindle/v1/schemas/*.schema.json` into `crates/spindle-cli/tests/schemas/`.
*   **Test**:
    ```rust
    #[test]
    fn test_query_output_validates_against_schema() {
        let output = run_spindle_query(...);
        validate_against_schema(&output, "tests/schemas/spindle.query.v1.schema.json");
    }
    ```
*   **Coverage**:
    *   `query`: provable, refuted, unknown.
    *   `requires`: satisfied, unsatisfied.
    *   (Future) `reason`, `explain`, `why-not`, `capabilities` once schemas exist.

**C. Exit Code Tests**

Create `crates/spindle-cli/tests/exit_code_tests.rs`.
Verify strict adherence to Section 8 of the contract:

1.  `query` returning `unknown` -> **Exit 0** (Logic outcome, not system failure).
2.  `requires` returning unsatisfied -> **Exit 0**.
3.  `explain` on unprovable literal -> **Exit 0** (currently fails with 1).
4.  Invalid file path / Syntax error -> **Exit 2**.
5.  Internal panic -> **Exit 3**.

**D. Determinism Tests**

Extend `reason_json_contract_tests.rs` patterns:

1.  **Sorting**: `conclusions`, `solutions`, `facts`, `trust.contributors` must be sorted.
2.  **Stability**: Run command N times with identical input; assert byte-identical JSON output.

### 6.2 Test Infrastructure

1.  Add `jsonschema = "0.17"` (or newer) to `dev-dependencies`.
2.  Keep shared test helpers in `crates/spindle-cli/tests/common/mod.rs` to deduplicate command execution and schema/error validation logic.

### 6.3 Matrix/Guard Strategy (Required to Avoid Regressions)

This is the anti-whackamole baseline. Contract changes are not complete until all three layers are updated.

1. Data-driven matrix coverage:
   - File: `crates/spindle-cli/tests/contract_matrix_tests.rs`
   - Model every command/edge case as `MatrixCase` entries.
   - Every `JsonSuccess` case must validate against the corresponding JSON schema, not ad-hoc required-field checks.
   - Success invariants must be schema-aware per command (for example, capabilities has a different shape than query/reason outputs).
2. Shared schema + runner helpers:
   - File: `crates/spindle-cli/tests/common/mod.rs`
   - Centralize binary invocation (`cargo_bin_cmd!`), schema loading, and strict JSON Schema validation.
   - Missing/unreadable schema files must fail fast (panic), never warn-and-skip.
   - JSON error envelope assertions (`diagnostics` + `error.code/message/details`) are centralized here.
3. Structural guard tests:
   - File: `crates/spindle-cli/tests/contract_guard_tests.rs`
   - Enforce boundary rules (no direct `std::process::exit` outside boundary emitter, no `eprintln!` in handlers, no deprecated `Command::cargo_bin` usage).
   - Test-file scans must recurse through nested test modules (for example, `tests/common/mod.rs`).
4. Authoring workflow for every contract fix:
   - Add or update matrix case first.
   - Add targeted guard assertion if the bug came from an architectural bypass.
   - Implement fix in CLI code.
   - Run `cargo test -p spindle-cli --test contract_matrix_tests --test contract_guard_tests`.

## 7. Non-Goals in This Plan

1. gleg workflow verification/gating logic.
2. UI semantics for pending/verified state.
3. gleg adapter throw/diagnostic policy details (covered in gleg doc).
