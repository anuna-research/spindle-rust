# Error Module Specification

| Field | Value |
|---|---|
| Document ID | SPEC-010 |
| Title | Dedicated Error Module for Consistent, Best-Practice Error Messages |
| Version | 0.1.0 |
| Status | Draft |
| Created | 2026-02-09 |
| Last Updated | 2026-02-09 |
| Authors | Codex (AI agent) |
| Reviewers | Core Maintainers, CLI Maintainers, Docs Maintainers |
| Parent | [SPINDLE Contract](./SPINDLE-CONTRACT.md) |

## 1. Executive Summary

This specification defines a dedicated Error module to centralize error modeling, classification, and message rendering across Spindle (core, CLI, WASM, and Rust integration). The module adopts the Problem Details pattern from [RFC 7807](https://datatracker.ietf.org/doc/html/rfc7807) for consistent structure, while preserving the existing JSON error envelope required by the Spindle contract. The result is uniform, predictable, and user-friendly errors across interfaces without breaking compatibility.

## 2. Feature Overview

Feature Name: error-module
Purpose: Provide a single canonical error model and rendering path across all Spindle interfaces.
User Story: As a user of Spindle (CLI, library, or WASM), I want errors to be consistent and actionable so that I can fix issues quickly and trust tooling behavior.
Business KPI Impact: Reduced support requests and faster developer turnaround on parse and validation errors.
Telemetry Spec (SLIs): error_render_latency_ms (p95), error_classification_accuracy (%)

Acceptance Criteria:

- [ ] Errors are generated through the new Error module in all user-facing interfaces.
- [ ] JSON error output conforms to existing contract requirements and includes RFC 7807-compatible fields.
- [ ] Human-readable errors follow a consistent, documented template.
- [ ] Error codes and exit codes are stable and traceable to requirements.
- [ ] Diagnostics remain available for non-fatal warnings and limit-truncation cases.

Data Classification: Internal
Privacy Notes: Error messages must avoid PII and redact sensitive file paths or system details unless explicitly requested.

## 3. Scope

In scope:

- Core error model, classification taxonomy, and rendering logic.
- CLI and JSON error formatting paths, aligned with [Error and Exit Behavior](./SPINDLE-CONTRACT.md#8-error-and-exit-behavior-normative).
- Rust and WASM integration surfaces using the shared model.
- Backward-compatible mapping from existing error types.

Out of scope:

- Localization and translation of error messages.
- Introduction of new error codes beyond the module's initial taxonomy.
- Changes to parser behavior or reasoning algorithms.

## 4. Background and Rationale

Spindle currently has multiple error pathways (core errors, parser errors, CLI errors) that produce different message shapes and levels of detail. The contract requires a specific JSON error envelope, but there is no single canonical model for error classification or message construction. Adopting RFC 7807's Problem Details pattern provides a well-known structure that cleanly separates a concise summary from actionable details and supports extension fields. This specification defines a dedicated module that centralizes error creation, enforces consistent message templates, and maps to both JSON and human-readable outputs.

## 5. Requirements

Functional Requirements:

- REQ-101: The system SHALL provide a canonical error model (`ProblemDetails`) that includes `type`, `title`, `status`, `detail`, and `instance` fields and supports extension members.
- REQ-102: The Error module SHALL be the sole pathway for constructing user-facing errors in CLI, Rust API, and WASM bindings.
- REQ-103: JSON error output SHALL conform to the contract envelope with `error.code`, `error.message`, `error.details`, and `diagnostics`, and SHALL include an embedded Problem Details object.
- REQ-104: The Error module SHALL map error categories to exit codes exactly as defined in the contract.
- REQ-105: The Error module SHALL provide deterministic, stable error codes and titles for all error categories.
- REQ-106: The Error module SHALL provide a human-readable rendering format that includes a concise summary, optional location (line/column), and a hint when available.
- REQ-107: The Error module SHALL redact sensitive details (absolute paths, OS error strings) unless a debug flag is explicitly enabled.
- REQ-108: The Error module SHALL support diagnostics for non-fatal conditions (e.g., truncation, timeouts) without converting them into errors.

Non-Functional Requirements:

- NFR-101: Error rendering SHALL add no more than 2 ms median overhead per operation under typical CLI usage.
- NFR-102: Error outputs SHALL be deterministic for the same inputs and environment (no randomized text or ordering).
- NFR-103: Error messages SHALL be stable across patch versions; changes require a major or minor spec version update.
- NFR-104: Error serialization SHALL be schema-versioned where required by the command contract.

Constraints:

- Technical: JSON output MUST preserve the current envelope shape required by the contract.
- Business: Error codes MUST remain stable for downstream tooling.
- Operational: Errors must be safe to log without leaking sensitive data by default.

## 6. Architecture Analysis

Affected Components:

- Data: Error model types and JSON serialization.
- Services: CLI command execution, parser, reasoning pipeline.
- Contracts: Error envelope and diagnostics in [SPINDLE Contract](./SPINDLE-CONTRACT.md#8-error-and-exit-behavior-normative).
- Presentation: Human-readable error messages for CLI output.

New Components:

- Error Module: Canonical error model, classification, and rendering functions.

Dependencies:

- Internal: `spindle-core` (error types), `spindle-cli` (renderers), `spindle-wasm` (bindings).
- External: [RFC 7807 Problem Details](https://datatracker.ietf.org/doc/html/rfc7807) pattern (structure and semantics).

Compatibility & Versioning:

- Backward compatible with existing JSON error envelope.
- Extension fields MAY evolve; breaking changes require a new `schema_version`.

## 7. Architecture Decisions

ADR-101: Adopt RFC 7807 Problem Details as the canonical internal error model.
Rationale: Provides a standard structure for error summaries, details, and extensions while remaining compatible with existing envelope requirements.
Alternatives Considered: Ad-hoc structs per interface; rejected due to inconsistency and duplicated logic.

ADR-102: Preserve the existing JSON error envelope and embed Problem Details within `error.details.problem`.
Rationale: Avoids breaking downstream consumers while adding structured error information.
Alternatives Considered: Replace envelope with Problem Details-only output; rejected due to contract incompatibility.

ADR-103: Use `urn:spindle:error:{CODE}` as the default `type` URI.
Rationale: Avoids dependency on external hosting and provides stable, unique identifiers.
Alternatives Considered: HTTPS URLs; rejected due to hosting and versioning overhead.

## 8. Contracts

CON-101: Error Module Interface

Endpoint/Interface: `spindle_core::error::ProblemDetails` and `ErrorReport`
Pre-conditions:

- An error event or diagnostic condition has been detected.

Post-conditions:

- A canonical error representation is produced with stable code and message fields.

Error model:

- `ProblemDetails` fields:
  - `type` (string, required): `urn:spindle:error:{CODE}`
  - `title` (string, required): short human-readable summary
  - `status` (integer, optional): exit code for CLI or `None` for non-fatal diagnostics
  - `detail` (string, optional): extended description, safe to show to end users
  - `instance` (string, optional): identifier for a specific occurrence (trace ID)
  - `extensions` (object, optional): free-form fields (line, column, hint, command, source)

Implements:

- REQ-101
- REQ-105
- REQ-106
- REQ-107

Verified by:

- TEST-101
- TEST-103

CON-102: CLI JSON Error Envelope (Compatibility Contract)

Endpoint/Interface: `spindle-cli --json` error output
Pre-conditions:

- A non-zero exit condition occurs.

Post-conditions:

- Output is a JSON object with `diagnostics` and `error`.
- The embedded Problem Details is present at `error.details.problem`.

Error model:

```json
{
  "schema_version": "spindle.result.v1",
  "diagnostics": [],
  "error": {
    "code": "DFL_PARSE_ERROR",
    "message": "DFL parse error",
    "details": {
      "problem": {
        "type": "urn:spindle:error:DFL_PARSE_ERROR",
        "title": "DFL parse error",
        "status": 2,
        "detail": "Unexpected token near '=>'.",
        "instance": "trace-01HXYZ",
        "extensions": {
          "line": 5,
          "column": 13,
          "hint": "Check the rule syntax near the arrow.",
          "source": "stdin"
        }
      }
    }
  }
}
```

Implements:

- REQ-103
- REQ-104

Verified by:

- TEST-102
- TEST-104

## 9. Error Taxonomy

The Error module defines a stable taxonomy:

- `PARSE_ERROR`: Input parsing failures (DFL/SPL).
- `VALIDATION_ERROR`: Structural or semantic validation failures.
- `EXECUTION_ERROR`: Reasoning or query execution failures.
- `RESOURCE_LIMIT`: Timeouts, size limits, or truncation errors.
- `INTERNAL_ERROR`: Unexpected failures with no safe recovery path.

Each taxonomy entry maps to:

- Exit code (2, 3, or 4).
- Stable `code` for the error envelope (e.g., `DFL_PARSE_ERROR`).
- `type` URI (e.g., `urn:spindle:error:DFL_PARSE_ERROR`).
- Default `title` and `detail` templates.

## 10. Rendering Rules

Human-readable CLI errors:

- Format: `Error: {title}` followed by optional `at line {line}, column {column}` and a `Hint: {hint}` line.
- `detail` is shown only when it provides actionable guidance.
- Sensitive details are redacted unless `--debug-errors` is enabled.

JSON errors:

- Always include `diagnostics`.
- `error.details.problem` is required.
- `status` reflects CLI exit code.

Diagnostics:

- Remain non-fatal; do not populate `error` when exit code is `0`.
- Must use stable diagnostic codes from the contract.

## 11. Testing Strategy

TEST-101: ProblemDetails construction preserves required fields and extension data.
TEST-102: CLI JSON error output contains envelope plus embedded Problem Details.
TEST-103: Error rendering redacts sensitive data unless `--debug-errors` is enabled.
TEST-104: Error taxonomy maps to exit codes and stable codes as specified.
TEST-105: Diagnostics-only cases (e.g., truncation) emit no `error` object.

## 12. Traceability

Trace links:

- REQ-101 → CON-101 → TEST-101
- REQ-102 → CON-101 → TEST-102
- REQ-103 → CON-102 → TEST-102
- REQ-104 → CON-102 → TEST-104
- REQ-105 → CON-101 → TEST-104
- REQ-106 → CON-101 → TEST-101
- REQ-107 → CON-101 → TEST-103
- REQ-108 → CON-102 → TEST-105

---

## Document History

| Version | Date | Author | Changes |
|---|---|---|---|
| 0.1.0 | 2026-02-09 | Codex (AI agent) | Initial draft |

---

**END OF SPECIFICATION**
