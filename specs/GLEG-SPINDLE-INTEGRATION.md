# gleg Spindle Adapter Integration Spec

## 1. Purpose and Audience

This document is for gleg maintainers.  
It defines how gleg should project inputs, call spindle CLI, and map outputs into core ports.

The wire contract comes from `specs/SPINDLE-CONTRACT.md`.

## 2. Boundary Responsibilities

Spindle owns:

1. Defeasible reasoning results.
2. Trust overlay scoring/explainability outputs.
3. Contracted CLI/schema behavior.

gleg owns:

1. Event-log projection into snapshot givens.
2. Human verification workflow state and gating.
3. Adapter error-handling policy and retries.

## 3. Reasoner Port vs Schema Gap

Current core port (`src/core/reasoner.ts`) is string-oriented:

1. `conclusions: string[]`
2. `missing: string[][]`

Contract outputs are structured (`literal_struct`, status metadata, trust object).

Decision:

1. Evolve the core port to structured types.
2. Keep a compatibility projection layer if legacy callers still need strings.

Rationale:

1. Lossy string projection hides provenance and structure needed for richer action proposals and explainability.
2. Preserving structured literals avoids adapter-only logic forks later.

Recommended migration:

1. Add `ReasonerResultV2` and `RequirementResultV2` with structured fields.
2. Keep existing string fields temporarily as derived convenience fields.
3. Remove string-only fields after consumers migrate.

## 4. Adapter Invocation Behavior

### 4.1 `status()`

1. Build theory/givens from projection.
2. Call `spindle reason --json`.
3. Preserve both `literal_spl` and `literal_struct` in mapped results.

### 4.2 `require()`

This operation is explicitly two-step unless spindle later provides an atomic operation:

1. Call `spindle query --json <goal>`.
2. If `status=provable`, return `satisfied=true, missing=[]`.
3. Else call `spindle requires --json <goal>`.
4. Return abductive solutions as `missing`.

This composition is intentional and part of gleg adapter behavior.
Both calls use the same generated input snapshot, so there is no cross-call state race.

### 4.3 `referenceTime` / `--at`

Policy:

1. Read spindle capabilities before use (cache optional for stateless service).
2. If `features.at=true`, pass `--at <referenceTime>`.
3. If `features.at=false`, emit warning diagnostic and continue without `--at`.

## 5. Adapter Error-Handling Policy

Define and keep consistent behavior across methods.

Recommended policy:

1. Domain outcomes never throw:
   - unknown status, unsatisfied requirements, empty proof tree.
2. Recoverable spindle command errors return diagnostics in result payloads.
3. Hard adapter failures throw only for:
   - local misconfiguration (missing binary),
   - malformed internal request construction,
   - unrecoverable parse corruption.

Examples of malformed internal request construction:

1. Empty/missing `goal` for `require()`.
2. Simultaneously attempting file and stdin theory modes in one invocation.
3. Non-serializable adapter-generated trust payload.

`status()` and `require()` should follow the same policy shape.

## 6. Projection and Input Normalization

Before invoking spindle:

1. Deduplicate givens by canonical literal identity.
2. Resolve event-log conflicts during projection (not in spindle request assembly).
3. Normalize ordering for deterministic snapshots.

For trust inputs:

1. Merge by `source_id`.
2. Enforce deterministic ordering of merged trust records.
3. Delegate conflicting-duplicate validation to spindle as authoritative contract enforcement, and always forward spindle diagnostics.
4. Optional preflight validation is allowed for UX, but must not replace or diverge from spindle validation outcomes.

## 7. Trust and Verification in gleg

1. Trust values from spindle are consumed for ranking/explainability.
2. Verification state is entirely gleg-managed.
3. Workflow gates like "all required facts must be human-verified" must not be inferred from spindle trust output.

## 8. Configuration

Required runtime config:

1. `SPINDLE_PATH` (default `spindle`)
2. `SPINDLE_TEMP_DIR` (optional)
3. `SPINDLE_DEBUG` (optional temp retention)

Recommended:

1. For stateless service deployment, no long-lived capability cache is required.
2. If a process cache is used, cache for process lifetime and restart to pick up spindle upgrades.
3. Add adapter-level timeout guard aligned with spindle `--timeout-ms`.

## 9. Integration Tests (gleg)

### 9.1 Test Strategy

We verify the adapter in two layers:

**A. Parser Unit Tests (Mocked)**

Location: `tests/adapters/spindle/` (unit tests).
Goal: Verify parser correctness and error handling without a spindle binary.

1.  **Fixtures**: Use JSON files conforming strictly to `contracts/spindle/v1/schemas/*.schema.json`.
2.  **`parseReasonOutput`**:
    *   Verify mapping of `literal_spl` and `literal_struct`.
    *   Verify filtering of non-defeasible conclusions (strict vs defeasible).
    *   Verify diagnostic mapping.
3.  **`parseRequireOutput`**:
    *   `satisfied=true` -> returns satisfied result.
    *   `satisfied=false` -> maps solutions correctly.
    *   Verify `score` field mapping.
4.  **`buildSpl`**:
    *   Verify correct SPL construction from theory + givens.
    *   Verify correct `(given ...)` wrapping.

**B. Integration Tests (Real Binary)**

Location: `tests/integration/spindle-cli.test.ts`.
Goal: Verify end-to-end contract adherence with the real binary.

1.  **Status Mapping**:
    *   `status()` correctly maps `provable`, `refuted`, `unknown`.
2.  **`require()` Two-Step Flow**:
    *   Prove short-circuit behavior (provable goal returns immediately).
    *   Prove abduction behavior (unprovable goal returns missing facts).
3.  **Temporal Support**:
    *   Verify `--at` is passed when capabilities allow.
    *   Verify warning emitted when capabilities deny.
4.  **Error Policy**:
    *   Verify `status()` returns diagnostics (doesn't throw) on recoverable errors.
    *   Verify hard failures (missing binary) throw.
5.  **Determinism**:
    *   Identical inputs -> identical outputs.

### 9.2 Test Infrastructure

1.  **Gating**: Tests needing the binary must skip unless `SPINDLE_PATH` is set or `SPINDLE_INTEGRATION=1`.
2.  **CI**: Dedicated job installs spindle binary to run integration tests.
3.  **Shared Fixtures**: Where possible, use the same JSON schema definitions as the Rust-side tests to ensure contract alignment.
