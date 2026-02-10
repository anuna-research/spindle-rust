# Implementation Plan: Spindle Simplification and Contract Cleanup

| Field | Value |
|---|---|
| Document ID | IMPL-010 |
| Title | Spindle Simplification Cleanup Plan (Repo-Aligned, Error-Module Ready) |
| Version | 1.0.0 |
| Status | Draft |
| Created | 2026-02-10 |
| Last Updated | 2026-02-10 |
| Authors | Codex (AI agent) |
| Reviewers | Core Maintainers, CLI Maintainers, WASM Maintainers |
| Parent Spec | [SPEC-010 Error Module](./ERROR-MODULE-SPEC.md) |
| Protocol Basis | [USDD Agent Protocol v1.0.0](../usdd-agent-protocol.md) |

## 1. Executive Summary

This plan replaces the prior cleanup plan with a repository-accurate implementation roadmap. The previous version referenced artifacts that do not exist in the current tree (for example `contract_matrix_tests.rs`, `contracts/spindle/v1/schemas/`, and `SPINDLE-CONTRACT.md`), so it could not be used as an execution guide.

The current codebase already includes major correctness work (AtomId/LitId reasoning identity, grounding pipeline, temporal as-of filtering, stable reason JSON output). Remaining cleanup work is now primarily about:

1. CLI structure simplification (`main.rs` decomposition and boundary cleanup).
2. Shared contract ownership between CLI and WASM.
3. Error module readiness and migration path for `SPEC-010`.
4. CI/docs alignment and contract-traceable tests.

## 2. Current-State Audit (Repository-Verified)

OBS-201: Reasoning correctness foundations are already implemented in core.
- Evidence: `crates/spindle-core/src/index.rs`, `crates/spindle-core/src/pipeline.rs`, `crates/spindle-core/src/reason.rs`, `crates/spindle-core/src/rule.rs`.
- Evidence: tests in `crates/spindle-core/tests/regression_tests.rs`, `crates/spindle-core/tests/query_arg_discrimination_tests.rs`, `crates/spindle-core/tests/temporal_asof_tests.rs`.
- Impact: cleanup should avoid re-opening correctness changes; focus on boundaries and structure.

OBS-202: CLI and WASM each define near-duplicate reason JSON transport structs.
- Evidence: `crates/spindle-cli/src/main.rs` (`ReasonOutput`, `GroundingStats`, `ConclusionStruct`, `TheoryStats`).
- Evidence: `crates/spindle-wasm/src/lib.rs` (`JsReasonOutput`, `JsGroundingStats`, `JsConclusionStruct`, `JsTheoryStats`).
- Impact: drift risk and duplicated schema evolution work.

OBS-203: CLI command orchestration and output rendering are monolithic.
- Evidence: `crates/spindle-cli/src/main.rs` contains argument parsing, IO, command dispatch, output formatting, and error handling.
- Impact: high change surface and difficult incremental migration to `SPEC-010`.

OBS-204: `run_reason` currently prepares theory, then calls `reason()` which prepares again.
- Evidence: `crates/spindle-cli/src/main.rs` calls `prepare(...)` then `spindle_core::reason::reason(&pipeline_result.theory)`.
- Evidence: `crates/spindle-core/src/reason.rs` `reason()` delegates to `reason_with_options(...)` which runs `prepare(...)`.
- Impact: unnecessary repeated pipeline work and multiple boundary paths.

OBS-205: Error model centralization has not started yet.
- Evidence: `crates/spindle-core/src/error.rs` and `crates/spindle-parser/src/error.rs` have basic enums without stable code/category APIs, no redaction model, no presentation-layer Problem Details object.
- Evidence: CLI and WASM mostly render `e.to_string()` via `eprintln!` or `JsError::new`.
- Impact: `SPEC-010` is not yet implemented; cleanup should stage this without destabilizing behavior.

OBS-206: Contract and docs references are partially stale.
- Evidence: `specs/ERROR-MODULE-SPEC.md` references `./SPINDLE-CONTRACT.md`, which is missing in repo.
- Evidence: `.woodpecker/release.yaml` uses `spindle reason theory.dfl --format json`, but CLI uses `--json`.
- Impact: planned work needs spec/doc hygiene before implementation waves.

OBS-207: Baseline test health is strong and should be preserved.
- Verified on 2026-02-10:
  - `cargo test -p spindle-core` (all passing)
  - `cargo test -p spindle-cli` (all passing)
  - `cargo test -p spindle-wasm` (all passing)
- Impact: cleanup must keep behavior stable while improving structure.

## 3. Goals and Non-Goals

### Goals

1. Provide a repo-accurate simplification plan with traceable requirements and tests.
2. Reduce boundary duplication across CLI/WASM by introducing shared contract types.
3. Split CLI boundary code into cohesive modules without changing user-facing behavior.
4. Prepare an implementation path for `SPEC-010` with incremental, testable milestones.
5. Align CI/docs/release examples with real CLI behavior and contract artifacts.

### Non-Goals

1. Rewriting core reasoning algorithms (DL(d), DL(d||)).
2. Introducing interval-set temporal inference (still out of scope vs current T1 as-of semantics).
3. Breaking current JSON success payload schema (`spindle.reason.v1`) during cleanup.

## 4. Requirements

REQ-201: The cleanup plan SHALL align only to files and test suites that exist in the repository at execution time.

Trace:
- TEST-201
- OBS-201

REQ-202: CLI command execution SHALL be refactored out of `crates/spindle-cli/src/main.rs` into dedicated modules (`app`, `input`, `output`, `error`, `commands`) while preserving CLI behavior and flags.

Trace:
- TEST-202
- CON-201
- OBS-203

REQ-203: Transport output types used by CLI and WASM for `reason` SHALL be owned by a shared crate/module to eliminate duplication and schema drift.

Trace:
- TEST-203
- CON-202
- OBS-202

REQ-204: Reasoning command paths SHALL execute `prepare` at most once per request unless explicitly configured otherwise for diagnostics.

Trace:
- TEST-204
- CON-203
- OBS-204

REQ-205: Error boundary rendering SHALL support a stable structured envelope compatible with current CLI contract fields (`error.code`, `error.message`, `error.details`, `diagnostics`) and add `error.details.problem` per `SPEC-010`.

Trace:
- TEST-205
- CON-204
- OBS-205

REQ-206: Error codes and category-to-exit-code mapping SHALL be deterministic and documented as stable API, while human-readable display text remains non-stable.

Trace:
- TEST-206
- CON-204
- OBS-205

REQ-207: CLI and WASM docs/release examples SHALL match implemented flags and output options (`--json`, not `--format json`).

Trace:
- TEST-207
- OBS-206

REQ-208: CI SHALL include explicit contract gates for structured output and error envelope behavior in addition to crate-wide tests.

Trace:
- TEST-208
- OBS-207

### Non-Functional Requirements

NFR-201: Cleanup refactors SHALL maintain green test suites for `spindle-core`, `spindle-cli`, and `spindle-wasm`.

NFR-202: Structured output ordering SHALL remain deterministic for identical inputs.

NFR-203: Boundary refactor SHALL not introduce measurable user-visible regression in standard CLI operations (target: no material regression under current benchmark scale).

## 5. Architecture Decisions

ADR-201: Introduce a shared contract crate (`crates/spindle-contract`) for transport-level DTOs and schemas used by CLI/WASM.

Rationale:
- Prevents duplicated JSON type definitions.
- Enables one place for schema/version evolution and error envelope types.

Alternatives considered:
- Keep duplicate structs in CLI and WASM (rejected: drift risk).
- Put DTOs in `spindle-core` (rejected: couples reasoning domain with presentation contract).

ADR-202: Keep typed library errors in `spindle-core` and `spindle-parser`; perform presentation conversion at boundaries (CLI/WASM), consistent with `SPEC-010`.

Rationale:
- Maintains idiomatic Rust library/application separation.
- Avoids pushing presentation concerns into reasoning core.

Alternatives considered:
- Use `ProblemDetails` as internal propagation error type (rejected: loses domain typing).

ADR-203: Introduce an explicit prepared-theory reasoning path to remove double-prepare execution in CLI.

Rationale:
- Eliminates redundant pipeline work.
- Improves clarity of where filtering/grounding happens.

Alternatives considered:
- Leave double prepare for simplicity (rejected: unnecessary cost and hidden behavior).

ADR-204: Stage `SPEC-010` rollout with compatibility-first migration.

Rationale:
- Existing users depend on current exit behavior and JSON structure.
- Incremental migration enables contract tests at each step.

Alternatives considered:
- Big-bang replacement of error handling (rejected: high regression risk).

ADR-205: Resolve missing contract documentation as a prerequisite hygiene task.

Rationale:
- `SPEC-010` references missing `SPINDLE-CONTRACT.md`; traceability and reviews require concrete source-of-truth docs.

## 6. Contract Definitions

CON-201: CLI Internal Command Boundary
- Interface: `run_<command>(CommandContext) -> Result<CommandOutput, CliBoundaryError>`
- Pre-conditions: parsed args are validated and source input is resolved.
- Post-conditions: command returns structured output; renderer handles final formatting and exit behavior.
- Implements: REQ-202, REQ-204.

CON-202: Shared Reason Output Contract (`spindle.reason.v1`)
- Required fields: `schema_version`, `grounding`, `conclusions`, `stats`.
- Optional field: `evaluated_at` (RFC3339 when `--at` is provided).
- Stable semantics:
  - `literal_spl` is canonical SPL s-expression.
  - `literal_struct` is semantic machine representation.
- Implements: REQ-203, REQ-208.

CON-203: Prepared-Theory Execution Contract
- Interface option A: `reason_prepared(prepared: &PreparedTheoryLike)`.
- Interface option B: `reason_on_theory(theory, options)` returns both conclusions and pipeline report once.
- Requirement: at most one `prepare` invocation per CLI reason request.
- Implements: REQ-204.

CON-204: Error Envelope Contract (SPEC-010-aligned)
- Envelope fields (stable): `error.code`, `error.message`, `error.details`, `diagnostics`.
- Extension: `error.details.problem` (RFC 9457 structure with `type`, `title`, `detail`, `instance`, extension members).
- Exit behavior: deterministic mapping by error category.
- Implements: REQ-205, REQ-206.

## 7. Test Specifications

TEST-201: Plan-to-repo consistency check
- Verify all referenced files/tests exist before each implementation wave.
- Verifies: REQ-201.

TEST-202: CLI behavior parity tests after module split
- Run existing CLI integration tests unchanged.
- Add smoke checks for no-arg/help/version and all subcommands.
- Verifies: REQ-202, NFR-201.

TEST-203: Shared DTO parity tests (CLI vs WASM)
- Serialize identical theory outcomes in CLI and WASM and assert schema/value parity.
- Verifies: REQ-203, NFR-202.

TEST-204: Single-prepare invariant tests
- Add instrumentation/unit tests ensuring reason path does not call `prepare` twice.
- Verifies: REQ-204, NFR-203.

TEST-205: Error envelope contract tests
- JSON error assertions for code/message/details/diagnostics/problem.
- Include redaction behavior and debug override cases.
- Verifies: REQ-205.

TEST-206: Error code stability tests
- Snapshot/approval tests for error code, title, and category-to-exit-code map.
- Verifies: REQ-206.

TEST-207: Docs/release command validity checks
- Validate examples in docs and release pipeline commands against current CLI flags.
- Verifies: REQ-207.

TEST-208: CI gate coverage checks
- Ensure CI explicitly runs contract suites (`reason_json_contract_tests`, `wasm_contract`, new error contract tests).
- Verifies: REQ-208.

## 8. Observability Signals

OBS-208: `contract_test_pass_rate` (CI signal for contract suites).
OBS-209: `error_envelope_validation_failures` (test-stage counter).
OBS-210: `prepare_invocation_count_per_reason_request` (debug/test instrumentation).
OBS-211: `json_contract_drift_events` (snapshot diff count in CI).

## 9. Work Plan (Phased)

### Phase A: Spec and Contract Hygiene

Scope:
- Reconcile missing references (`SPINDLE-CONTRACT.md` presence or updated links).
- Correct stale docs/pipeline examples (`--json` usage).
- Finalize contract ownership location (`spindle-contract` crate plan).

Exit Criteria:
- Spec links resolve.
- CLI docs and release examples match implementation.

### Phase B: CLI Boundary Decomposition (No Behavior Change)

Scope:
- Split `crates/spindle-cli/src/main.rs` into:
  - `src/cli/app.rs`
  - `src/cli/input.rs`
  - `src/cli/output.rs`
  - `src/cli/error.rs`
  - `src/cli/commands/*.rs`
- Keep existing command flags and stdout/stderr behavior.

Exit Criteria:
- All current CLI tests green with no output regressions.

### Phase C: Shared Contract Extraction

Scope:
- Create `crates/spindle-contract` for shared reason output DTOs.
- Migrate CLI and WASM reason serialization to shared types.
- Optionally add schemas under `contracts/spindle/v1/` if team wants generated/checked schema artifacts.

Exit Criteria:
- No duplicate DTO definitions in CLI and WASM for reason output.
- Existing contract tests pass, plus parity tests.

### Phase D: SPEC-010 Enablement (Error Module)

Scope:
- Add stable error code/category metadata in typed errors.
- Implement boundary renderer for Problem Details embedding inside current envelope.
- Add redaction defaults and debug override behavior.

Exit Criteria:
- Error envelope contract tests pass.
- CLI/WASM error rendering behavior is consistent and deterministic.

### Phase E: CI and Test Suite Simplification

Scope:
- Explicitly gate contract suites in CI.
- Reduce redundant test overlap where behavior is already covered at lower levels.
- Keep integration-first bias for end-user behavior.

Exit Criteria:
- CI clearly signals contract failures vs general unit failures.
- Test maintenance burden reduced without coverage loss.

## 10. Risks and Mitigations

Risk-1: Error-module rollout introduces downstream JSON breakages.
- Mitigation: compatibility envelope retained; add extension fields instead of replacing structure.

Risk-2: CLI refactor drifts behavior.
- Mitigation: lock behavior with existing integration tests and command snapshots.

Risk-3: Shared contract crate creates dependency coupling issues.
- Mitigation: keep crate transport-only (no reasoning logic), with strict API boundaries.

Risk-4: `SPEC-010` assumptions conflict with current crate dependency graph.
- Mitigation: resolve architecture in ADR before coding (do not force cyclic dependencies between parser/core).

## 11. Recommended PR Split

1. PR-1: Phase A (spec/doc/reference hygiene + CI example fixes).
2. PR-2: Phase B (CLI module decomposition only, no contract changes).
3. PR-3: Phase C (shared contract crate + CLI/WASM adoption).
4. PR-4: Phase D (error module boundary implementation + contract tests).
5. PR-5: Phase E (CI contract gate explicitness + test suite deduplication).

## 12. Quality Gates (USDD-Conformant)

- [ ] All REQ items map to TEST and at least one CON/OBS where applicable.
- [ ] No unresolved document references.
- [ ] Contract-breaking changes are explicitly versioned.
- [ ] CLI/WASM outputs remain deterministic.
- [ ] Security/privacy defaults for error rendering are validated.
- [ ] End-to-end crate tests remain green.
