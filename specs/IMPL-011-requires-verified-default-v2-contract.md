# Verified `requires` by Default + `spindle.requires.v2`

| Field | Value |
|---|---|
| Document ID | IMPL-011 |
| Title | Verified `requires` by Default + v2 Contract Rollout |
| Version | 1.0.1 |
| Status | Draft |
| Created | 2026-02-26 |
| Last Updated | 2026-02-26 |
| Authors | GPT-5 Codex (AI agent) |
| Reviewers | Core Maintainers |
| Protocol | [USDD Agent Protocol v1.0.0](../../usdd-agent-protocol.md) |

---

## 1. Executive Summary

This plan fixes a known correctness gap in `requires`: raw abduction candidates can include fact-sets that do not make the goal provable once defeaters and superiority are applied. The fix is to make `requires` verified-by-default by re-checking each raw candidate against full reasoning.

The rollout includes a new CLI JSON contract version (`spindle.requires.v2`) with explicit verification metadata and bounded-search status fields. `--fast` is intentionally omitted in this phase. Behavior is bounded and deterministic.

This is a forward change targeted for `0.2.0`.

---

## 2. Problem Statement and Context

### 2.1 Current Correctness Problem

BUG 5 documents that some abduced candidates are semantically invalid under full defeasible reasoning:

- File: `crates/spindle-core/tests/regression_known_bugs.rs`
- Existing behavior: candidate facts can be produced by `abduce` but fail when injected and re-reasoned.
- Practical impact: consumers using `requires` as a derivation surface can see false positives and miss validated candidates after truncation.

### 2.2 Current Surface Mismatch

`requires` semantics differ by surface:

- Core API (`crates/spindle-core/src/query/requires.rs`) is single-solution.
- CLI (`crates/spindle-cli/src/cli/commands/requires.rs`) is multi-solution and emits v1 JSON.

This mismatch complicates deterministic policy and makes correctness guarantees unclear.

### 2.3 Contract Constraint Conflict in v1

`spindle.requires.v1` enforces `satisfied=false => solutions.minItems=1`. Verified semantics require allowing `satisfied=false` with `solutions=[]` when all raw candidates fail verification.

---

## 3. Scope

| In Scope | Out of Scope |
|---|---|
| Verified-by-default `requires` behavior in core and CLI | Any unverified `--fast` mode |
| New core multi-solution options API | Completeness mode (`--complete`) |
| `spindle.requires.v2` JSON contract | WASM `requires` bindings |
| Capabilities update to advertise v2 | Dynamic ranking/scoring model |
| Deterministic ordering + dedup | Changes to raw `abduce` semantics |
| Regression and contract test updates | Non-CLI remote/service transport |

---

## 4. Architecture Decisions

### ADR-001: `requires` is verified by default

Decision:
- Every raw candidate considered by `requires` is verified via full reasoning on a cloned theory with injected candidate facts.

Rationale:
- Fixes known correctness defect without changing `abduce` internals.

Trade-off:
- Higher cost (up to one `reason()` pass per examined candidate), bounded by explicit budget.

### ADR-002: Preserve `satisfied` semantics

Decision:
- `satisfied` keeps v1 meaning: goal already provable without added facts.
- Add explicit `already_provable` in core result type.
- In v2 JSON, allow `satisfied=false` and `solutions=[]`.

Rationale:
- Avoids semantic drift for existing consumers while resolving the verified-empty result case.

### ADR-003: Bounded search with explicit status

Decision:
- Add `max_raw_candidates` budget (default: `1000`), separate from `max_solutions`.
- `requires_with_options` requests `max_raw_candidates + 1` raw candidates from `abduce` to detect truncation via sentinel.
- `search_status` meanings:
  - `BoundedComplete`: search terminated within configured bounds (all raw candidates exhausted, or `max_solutions` reached).
  - `BudgetExhausted`: raw-candidate budget was reached and additional raw candidates exist beyond the examined set.

Rationale:
- Prevents unbounded verification cost and makes truncation explicit.

### ADR-004: No `--fast` in this phase

Decision:
- CLI exposes one mode only: verified-by-default.

Rationale:
- One correctness story for `0.2.0`, less surface complexity.

### ADR-005: Core compatibility helper becomes verified

Decision:
- Deprecated `requires()` becomes a thin wrapper over verified `requires_with_options` with `max_solutions=1` and default budget.

Rationale:
- Eliminates split correctness behavior inside core API while preserving source-level compatibility for callers.

### ADR-006: WASM out of scope for this phase

Decision:
- `spindle-wasm` continues exposing existing unverified abduction surface in this release.

Rationale:
- Constrains rollout scope and avoids widening blast radius.

---

## 5. Functional Requirements

### REQ-001: Verified candidate acceptance

The system SHALL verify each raw candidate fact-set by:
1. Cloning theory.
2. Injecting candidate facts.
3. Running full `reason()`.
4. Accepting candidate only when goal is provable in the resulting conclusions.

Trace:
- CON-001
- TEST-001
- TEST-007

### REQ-002: New multi-solution core API

The system SHALL expose `requires_with_options(theory, goal, options)` returning:
- `already_provable: bool`
- verified `solutions`
- verification counters
- bounded `search_status`

Trace:
- CON-001
- TEST-002
- TEST-003

### REQ-003: Deprecated `requires()` wrapper semantics

The deprecated `requires()` API SHALL return verified results by delegating to `requires_with_options` with `max_solutions=1` and default budget.

Trace:
- CON-001
- TEST-004

### REQ-004: Deterministic dedup and ordering

The system SHALL deduplicate candidate sets by canonical fact-key and order verified solutions deterministically:
1. Fewer facts first.
2. Lexical order of canonical fact keys.

Trace:
- CON-001
- TEST-005

### REQ-005: Two independent bounds

The system SHALL terminate search when either condition is met:
- verified accepted solutions reach `max_solutions`, or
- examined raw candidates reach `max_raw_candidates`.

Trace:
- CON-001
- TEST-003

### REQ-006: CLI `requires --json` uses v2 only

The CLI SHALL emit only `spindle.requires.v2` for `requires --json` in this release.

Trace:
- CON-002
- TEST-006

### REQ-007: Preserve `satisfied` semantic meaning

The CLI and contract SHALL keep `satisfied` meaning "already provable without additions".

Trace:
- CON-002
- TEST-002

### REQ-008: v2 verification metadata

The v2 JSON payload SHALL include:
- `verification_mode`
- `search_status`
- `verification.raw_examined`
- `verification.accepted`
- `verification.rejected`

Trace:
- CON-002
- OBS-001
- TEST-006

### REQ-009: Capabilities advertise v2

`spindle capabilities --json` SHALL report `schemas.requires = "spindle.requires.v2"`.

Trace:
- CON-003
- TEST-006
- OBS-002

### REQ-010: No fast-mode option

The CLI SHALL NOT expose a `--fast`/unverified mode in this phase.

Trace:
- CON-002
- TEST-006

### REQ-011: Score stability

`score` SHALL remain `1.0` placeholder in v2. Ranking signal remains array order.

Trace:
- CON-002
- TEST-005

### REQ-012: Bug regression conversion

BUG 5 regression coverage SHALL be converted from known-bug documentation to passing verification assertions for the new `requires` path, while preserving explicit raw-abduce behavior coverage.

Trace:
- TEST-007

---

## 6. Non-Functional Requirements

### NFR-001: Bounded compute cost

`requires` SHALL enforce a default bounded raw-candidate budget of `1000` when not overridden by options.

Trace:
- CON-001
- TEST-003
- OBS-001

### NFR-002: Deterministic output reproducibility

Given same theory, goal, and options, solution ordering and counters SHALL be deterministic.

Trace:
- TEST-005
- OBS-001

### NFR-003: Forward migration clarity

CLI schema transition SHALL be explicit through capabilities and docs in the same release.

Trace:
- CON-003
- TEST-006
- OBS-002

---

## 7. Contract Specifications

### CON-001: Core `requires_with_options` contract

Interface:
- File: `crates/spindle-core/src/query/requires.rs`

```rust
pub struct RequiresOptions {
    pub max_solutions: usize,
    pub max_raw_candidates: usize, // default 1000
}

pub enum RequiresSearchStatus {
    // Search terminated within configured bounds:
    // either all raw candidates were exhausted, or max_solutions was reached.
    BoundedComplete,
    // Raw candidate budget hit while additional raw candidates still exist.
    BudgetExhausted,
}

pub struct RequiresVerificationStats {
    pub raw_examined: usize,
    pub accepted: usize,
    pub rejected: usize,
}

pub struct RequiresResult {
    pub already_provable: bool,
    pub solutions: Vec<AbductionSolution>,
    pub search_status: RequiresSearchStatus,
    pub verification: RequiresVerificationStats,
}

pub fn requires_with_options(
    theory: &Theory,
    goal: &Literal,
    options: RequiresOptions,
) -> Result<RequiresResult>;
```

Pre-conditions:
- `max_solutions >= 1`.
- `max_raw_candidates >= 1`.

Post-conditions:
- `solutions` contains only verified candidates.
- `verification.raw_examined == verification.accepted + verification.rejected`.
- `verification.raw_examined <= options.max_raw_candidates`.
- `search_status=BudgetExhausted` iff additional raw candidates exist beyond the examined set.
- `requires()` wrapper is verified and returns first solution facts or empty when none accepted.

Error model:
- Returns existing query errors for invalid inputs or reasoning failures.

Implements:
- REQ-001, REQ-002, REQ-003, REQ-004, REQ-005

Verified by:
- TEST-001, TEST-002, TEST-003, TEST-004, TEST-005

### CON-002: CLI JSON v2 contract

Interface:
- File: `contracts/spindle/v1/schemas/spindle.requires.v2.schema.json`
- Emitter: `crates/spindle-cli/src/cli/commands/requires.rs`
- Error envelope schema routing: `crates/spindle-cli/src/main.rs`

Required fields:
- `schema_version: "spindle.requires.v2"`
- `query`
- `satisfied`
- `solutions`
- `verification_mode: "verified"`
- `search_status: "BoundedComplete" | "BudgetExhausted"`
- `verification: { raw_examined, accepted, rejected }`
- Optional warnings array for truncation metadata

Important semantic rule:
- `satisfied` means already provable without added facts.
- `satisfied=false` with empty `solutions` is valid in v2.

Score rule:
- `solutions[*].score == 1.0` for now.

Implements:
- REQ-006, REQ-007, REQ-008, REQ-010, REQ-011

Verified by:
- TEST-002, TEST-005, TEST-006

### CON-003: Capabilities schema advertisement

Interface:
- File: `crates/spindle-cli/src/cli/commands/capabilities.rs`
- File: `contracts/spindle/v1/schemas/spindle.capabilities.v1.schema.json`

Rule:
- `schemas.requires` must advertise `spindle.requires.v2` in both runtime output and capabilities schema constraints.

Implements:
- REQ-009

Verified by:
- TEST-006

---

## 8. Observability Signals

### OBS-001: Verification effort visibility

The v2 `verification` block SHALL expose `raw_examined`, `accepted`, and `rejected` so consumers can distinguish:
- clean bounded completion,
- budget exhaustion,
- all-candidate rejection.

### OBS-002: Contract discoverability

Capabilities JSON SHALL expose the active requires schema version to support contract negotiation by clients.

---

## 9. Detailed Implementation Plan

## Track 1: Core verified requires

1. Add new options/result/status/stats types in `crates/spindle-core/src/query/requires.rs`.
2. Implement candidate verification function (clone + inject + re-reason + goal check).
3. Implement `requires_with_options` pipeline:
   - request `max_raw_candidates + 1` raw candidates from `abduce` (sentinel),
   - if sentinel exists, set `search_status=BudgetExhausted` and examine only first `max_raw_candidates`,
   - verify each candidate,
   - apply dedup,
   - deterministic sort,
   - enforce `max_solutions`,
   - compute counters + status.
4. Update deprecated `requires()` to call `requires_with_options` with `max_solutions=1` and default budget.
5. Keep raw `abduce` unchanged.

Done criteria:
- New API compiles and is documented.
- Deprecated wrapper returns verified behavior.
- Existing callsites continue to compile.

## Track 2: CLI and schema v2 rollout

1. Update `crates/spindle-cli/src/cli/commands/requires.rs` to call core verified API.
2. Emit only `spindle.requires.v2` in JSON mode.
3. Add v2 schema file at `contracts/spindle/v1/schemas/spindle.requires.v2.schema.json`.
4. Update `crates/spindle-cli/src/main.rs` schema routing for `Commands::Requires` from `spindle.requires.v1` to `spindle.requires.v2` so JSON error envelopes match v2.
5. Keep v1 requires schema file for historical/reference docs only; no runtime v1 output path.
6. Update capabilities output in `crates/spindle-cli/src/cli/commands/capabilities.rs`.
7. Update `contracts/spindle/v1/schemas/spindle.capabilities.v1.schema.json` so `schemas.requires.const = "spindle.requires.v2"`.
8. Keep `--max`; do not introduce `--fast` or `max_raw_candidates` CLI flag in this phase.

Done criteria:
- CLI JSON validates against v2 schema.
- CLI `requires --json` success and error envelopes both report `schema_version=spindle.requires.v2`.
- Capabilities reports v2.
- Capabilities output validates against updated capabilities schema.
- No fast/unverified mode exposed.

## Track 3: Tests and regressions

1. Update/add core tests in:
   - `crates/spindle-core/tests/regression_known_bugs.rs`
   - `crates/spindle-core/tests/proptest_query.rs`
   - `crates/spindle-core/tests/query_arg_discrimination_tests.rs`
   - `crates/spindle-core/tests/requires_verified_tests.rs` (new)
2. Add coverage for:
   - defeater candidate rejection,
   - already provable path,
   - budget exhausted path,
   - empty verified results,
   - deterministic order and dedup.
3. Update CLI schema test registry in `crates/spindle-cli/tests/common/mod.rs` to include `spindle.requires.v2.schema.json` (e.g., `requires_v2` entry), and validate requires matrix cases against v2.
4. Update CLI tests to validate v2 shape, capabilities schema version, and `requires` JSON error-envelope `schema_version`.
5. Ensure deprecated wrapper semantics are explicitly tested.

Done criteria:
- BUG 5 is converted into passing verified regression checks.
- No remaining tests assume v1-only runtime output.

## Track 4: Docs and release hygiene

1. Update docs:
   - `docs/src/reference/cli.md`
   - `docs/src/guides/queries.md`
2. Update contract docs references under `contracts/spindle/**` as needed.
3. Bump workspace version to `0.2.0` in `Cargo.toml`.
4. Ensure release notes mention behavior change: core `requires()` now verified.

Done criteria:
- Docs match shipped CLI behavior.
- Version bump and release notes aligned.

---

## 10. Test Specifications

### TEST-001: Defeater-blocked candidate rejection

Given a theory where a raw candidate is blocked by defeaters,
when `requires_with_options` executes,
then blocked candidate is rejected and does not appear in `solutions`.

Validates:
- REQ-001

### TEST-002: Already provable semantics

Given a goal already provable in base theory,
when `requires --json` runs,
then `satisfied=true`, `solutions=[]`, and verification counters are zero.

Validates:
- REQ-002, REQ-007, CON-002

### TEST-003: Budget exhaustion

Given a theory requiring many raw candidates with high rejection,
when `max_raw_candidates` is reached before `max_solutions`,
then `search_status=BudgetExhausted` and counters reflect examined candidates.

Validates:
- REQ-005, NFR-001

### TEST-004: Deprecated wrapper parity

Given any theory/goal,
when deprecated `requires()` is called,
then returned facts equal first verified solution from `requires_with_options(max_solutions=1)`.

Validates:
- REQ-003

### TEST-005: Deterministic order + score stability

Given multiple accepted candidates,
when `requires_with_options` runs repeatedly,
then solution order is stable (size then lexical), dedup is stable, and `score==1.0`.

Validates:
- REQ-004, REQ-011, NFR-002

### TEST-006: v2 contract and capabilities

Given `spindle requires --json` (success and error paths) and `spindle capabilities --json`,
then requires output validates v2 schema, requires error envelopes report `schema_version=spindle.requires.v2`, and capabilities advertises `spindle.requires.v2`.

Validates:
- REQ-006, REQ-008, REQ-009, REQ-010, NFR-003

### TEST-007: Known bug conversion

Given BUG 5 theory fixture,
then new `requires` path passes verified assertions, while raw abduce behavior remains explicitly documented.

Validates:
- REQ-012

---

## 11. Migration and Compatibility Notes

1. Semver:
- This is a behavior change release; target `0.2.0`.

2. CLI JSON output:
- Runtime emits v2 only for `requires` success and error envelopes.
- v1 schema remains in repository as historical reference.
- `spindle.requires.v2.schema.json` is colocated under `contracts/spindle/v1/schemas/` for compatibility with current schema test registry loading.

3. Core API behavior:
- `requires()` remains available but now verified-by-default via wrapper.
- Callers needing raw candidates must use `abduce` directly.

4. WASM:
- No verified `requires` WASM binding in this phase.

---

## 12. Performance Notes

Verification introduces up to one `reason()` pass per examined raw candidate. Worst-case work is bounded by `max_raw_candidates`.

Default budget (`1000`) balances correctness and bounded runtime for this phase. If profiling shows pressure, future work can tune default and/or add explicit advanced flags.

---

## 13. Risk Register

| Risk | Impact | Mitigation |
|---|---|---|
| Runtime cost regression on large theories | Medium | Enforce budget, monitor counters, benchmark follow-up |
| Consumer breakage from v2-only JSON | Medium | Capabilities advertises v2; docs updated in same release |
| Confusion about `satisfied` semantics | Medium | Preserve v1 meaning; document explicitly; add `already_provable` in core |
| Accidental reintroduction of unverified path | Low | No `--fast` option in this phase; tests assert verified behavior |

---

## 14. Definition of Done

1. Core verified API and wrapper behavior implemented.
2. CLI emits v2 only and capabilities advertise v2.
3. Regression and contract tests updated and passing.
4. Docs updated to match behavior.
5. Version bump to `0.2.0` prepared.
6. Verification gates pass:
   - `make check`
   - `make test`
