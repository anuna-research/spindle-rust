# Error Module Specification

| Field | Value |
|---|---|
| Document ID | SPEC-010 |
| Title | Dedicated Error Module for Consistent, Best-Practice Error Messages |
| Version | 0.2.0 |
| Status | Draft |
| Created | 2026-02-09 |
| Last Updated | 2026-02-10 |
| Authors | Codex (AI agent) |
| Reviewers | Core Maintainers, CLI Maintainers, Docs Maintainers |
| Parent | [SPINDLE Contract](./SPINDLE-CONTRACT.md) |

## 1. Executive Summary

This specification defines a dedicated Error module to centralize error modeling, classification, and message rendering across Spindle (core, CLI, WASM, and Rust integration). The module adopts the Problem Details pattern from [RFC 9457](https://www.rfc-editor.org/rfc/rfc9457.html) (which obsoletes [RFC 7807](https://datatracker.ietf.org/doc/html/rfc7807)) for consistent structure, while preserving the existing JSON error envelope required by the Spindle contract. The result is uniform, predictable, and user-friendly errors across interfaces without breaking compatibility.

The design follows Rust's idiomatic error handling conventions: library crates (`spindle-core`, `spindle-parser`) define typed error enums and propagate them via `Result<T, E>` and the `?` operator; presentation crates (`spindle-cli`, `spindle-wasm`) convert those errors into `ProblemDetails` for rendering. This separation respects the library-vs-application boundary described in [The Rust Programming Language, Chapter 9](https://doc.rust-lang.org/book/ch09-00-error-handling.html).

## 2. Feature Overview

Feature Name: error-module
Purpose: Provide a single canonical error model and rendering path across all Spindle interfaces.
User Story: As a user of Spindle (CLI, library, or WASM), I want errors to be consistent and actionable so that I can fix issues quickly and trust tooling behavior.
Business KPI Impact: Reduced support requests and faster developer turnaround on parse and validation errors.
Telemetry Spec (SLIs): error_render_latency_ms (p95), error_classification_accuracy (%)

Acceptance Criteria:

- [ ] Library crates return typed `Result` values using existing error enums with stable classification metadata.
- [ ] Presentation crates convert library errors into `ProblemDetails` at the boundary.
- [ ] JSON error output conforms to existing contract requirements and includes RFC 9457-compatible fields.
- [ ] Human-readable errors follow a consistent, documented template.
- [ ] Error codes and exit codes are stable and traceable to requirements.
- [ ] Diagnostics remain available for non-fatal warnings and limit-truncation cases.

Data Classification: Internal
Privacy Notes: Error messages must avoid PII and redact sensitive file paths or system details unless explicitly requested.

## 3. Scope

In scope:

- Core error taxonomy, classification metadata, and `From` conversions in `spindle-core`.
- Error rendering (`ProblemDetails`, `ErrorReport`) and JSON/human-readable formatting in `spindle-cli` and `spindle-wasm`.
- CLI and JSON error formatting paths, aligned with [Error and Exit Behavior](./SPINDLE-CONTRACT.md#8-error-and-exit-behavior-normative).
- Backward-compatible migration from existing `SpindleError` and `ParseError` types.

Out of scope:

- Localization and translation of error messages.
- Introduction of new error codes beyond the module's initial taxonomy.
- Changes to parser behavior or reasoning algorithms.

## 4. Background and Rationale

Spindle currently has multiple error pathways (core errors, parser errors, CLI errors) that produce different message shapes and levels of detail. The contract requires a specific JSON error envelope, but there is no single canonical model for error classification or message construction.

### Current Error Types

The codebase defines two `thiserror`-derived error enums:

- `SpindleError` in `spindle-core/src/error.rs`: `RuleNotFound`, `InvalidLiteral`, `TheoryError`, `ReasoningError`, `Validation`.
- `ParseError` in `spindle-parser/src/error.rs`: `LexerError`, `ParserError`, `UnexpectedToken`, `IoError`.

These types are well-suited for internal error propagation via the `?` operator. What is missing is a uniform rendering layer that maps these typed errors to structured, user-facing output.

### Why RFC 9457

Adopting RFC 9457's Problem Details pattern provides a well-known structure that cleanly separates a concise summary from actionable details and supports extension fields. RFC 9457 obsoletes RFC 7807 and adds a Problem Types registry, explicit guidance for non-dereferenceable URIs, and multiple-problem handling recommendations.

This specification uses RFC 9457 as the structural model for rendered output while keeping the existing Rust error enums as the internal representation.

## 5. Requirements

Functional Requirements:

- REQ-101: The system SHALL provide a presentation-layer error model (`ProblemDetails`) that includes `type`, `title`, `detail`, and `instance` fields and supports extension members, following the structure defined in [RFC 9457 Section 3](https://www.rfc-editor.org/rfc/rfc9457.html#section-3).
- REQ-102: Presentation crates (`spindle-cli`, `spindle-wasm`) SHALL convert library error types into `ProblemDetails` at the crate boundary. Library crates (`spindle-core`, `spindle-parser`) SHALL continue to return typed `Result<T, E>` values.
- REQ-103: JSON error output SHALL conform to the contract envelope with `error.code`, `error.message`, `error.details`, and `diagnostics`, and SHALL include an embedded Problem Details object at `error.details.problem`.
- REQ-104: The Error module SHALL map error categories to exit codes exactly as defined in the contract, using the `exit_code` extension member (not the RFC 9457 `status` field).
- REQ-105: The Error module SHALL provide deterministic, stable error codes and titles for all error categories. Per RFC 9457, `title` values SHALL remain constant across occurrences of the same problem type.
- REQ-106: The Error module SHALL provide a human-readable rendering format that includes a concise summary, optional location (line/column), and a hint when available.
- REQ-107: The Error module SHALL redact sensitive details (absolute paths, OS error strings) unless a debug flag is explicitly enabled.
- REQ-108: The Error module SHALL support diagnostics for non-fatal conditions (e.g., truncation, timeouts) without converting them into errors.
- REQ-109: Library error types SHALL implement `std::error::Error` (via `thiserror`) with correct `source()` chains to preserve causal context during propagation.
- REQ-110: The Error module SHALL provide `From` trait implementations to enable `?`-based conversion between library error types and the presentation `ProblemDetails` type.

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

### 6.1. Layer Separation

The error system is split into two layers following Rust's library-vs-application convention:

**Core layer** (`spindle-core`, `spindle-parser`) — defines typed error enums, implements `std::error::Error` and `Display` via `thiserror`, propagates errors with `?`:

```
spindle-parser::ParseError  ──?──▶  spindle-core::SpindleError  ──?──▶  caller
                                         (via From impl)
```

**Presentation layer** (`spindle-cli`, `spindle-wasm`) — converts core errors into `ProblemDetails` at the boundary, renders to JSON or human-readable output:

```
SpindleError  ──From──▶  ProblemDetails  ──render──▶  JSON / stderr
ParseError    ──From──▶  ProblemDetails  ──render──▶  JSON / stderr
```

### 6.2. Affected Components

- Data: Error model types and JSON serialization.
- Services: CLI command execution, parser, reasoning pipeline.
- Contracts: Error envelope and diagnostics in [SPINDLE Contract](./SPINDLE-CONTRACT.md#8-error-and-exit-behavior-normative).
- Presentation: Human-readable error messages for CLI output.

### 6.3. New Components

- `spindle-core::error` (extended): Classification metadata on existing error variants, `From<ParseError> for SpindleError` conversion.
- `spindle-cli::error_report`: `ProblemDetails`, `ErrorReport`, `Diagnostic` types and rendering functions.

### 6.4. Dependencies

- Internal: `spindle-core` (error types), `spindle-parser` (error types), `spindle-cli` (renderers), `spindle-wasm` (bindings).
- External: [RFC 9457 Problem Details](https://www.rfc-editor.org/rfc/rfc9457.html) pattern (structure and semantics), [`thiserror`](https://docs.rs/thiserror) (derive macros), [`serde`](https://docs.rs/serde) (JSON serialization).

### 6.5. Compatibility & Versioning

- Backward compatible with existing JSON error envelope.
- Extension fields MAY evolve; breaking changes require a new `schema_version`.
- Existing `Result<T, SpindleError>` and `Result<T, ParseError>` return types are unchanged.

## 7. Architecture Decisions

ADR-101: Adopt RFC 9457 Problem Details as the canonical presentation-layer error model.
Rationale: Provides a standard structure for error summaries, details, and extensions while remaining compatible with existing envelope requirements. RFC 9457 obsoletes RFC 7807 and is the current standard.
Alternatives Considered: Ad-hoc structs per interface; rejected due to inconsistency and duplicated logic.

ADR-102: Preserve the existing JSON error envelope and embed Problem Details within `error.details.problem`.
Rationale: Avoids breaking downstream consumers while adding structured error information.
Alternatives Considered: Replace envelope with Problem Details-only output; rejected due to contract incompatibility.

ADR-103: Use `tag:spindle.dev,2026:error:{CODE}` as the default `type` URI.
Rationale: RFC 9457 Section 3.1.1 explicitly discusses non-dereferenceable URIs. The `tag:` URI scheme ([RFC 4151](https://www.rfc-editor.org/rfc/rfc4151.html)) provides stable, unique identifiers without requiring hosted documentation. When `type` is absent, the default value is `"about:blank"` per RFC 9457, and in that case `title` SHOULD convey the same semantics as the exit code description.
Alternatives Considered: `urn:spindle:error:{CODE}` (valid but URN registration overhead); HTTPS URLs (hosting and versioning overhead).

ADR-104: Keep existing `thiserror`-derived error enums as the internal error types; convert to `ProblemDetails` only at the presentation boundary.
Rationale: Rust's error handling idiom is that libraries return typed `Result` values and applications decide how to present them. This preserves `?`-based propagation, `source()` chains, and pattern matching on error variants throughout the core and parser crates. `ProblemDetails` is a rendering concern, not a propagation concern.
Alternatives Considered: Replace all error types with `ProblemDetails` everywhere; rejected because it loses type safety, breaks pattern matching, and conflates library and application responsibilities.

ADR-105: Use extension members for CLI exit codes instead of the RFC 9457 `status` field.
Rationale: RFC 9457 defines `status` as the HTTP status code generated by the origin server. Spindle is not an HTTP API — repurposing `status` for CLI exit codes would violate the RFC's semantics and confuse consumers familiar with Problem Details. CLI exit codes are carried in `extensions.exit_code` instead. If Spindle later exposes an HTTP API, `status` can be populated with actual HTTP status codes at that time.
Alternatives Considered: Repurpose `status` for exit codes; rejected due to semantic mismatch with RFC 9457.

## 8. Rust Implementation Model

### 8.1. Core Error Types (unchanged, extended with metadata)

The existing error enums remain the primary error types for internal use. Each variant gains classification metadata via a method:

```rust
// spindle-core/src/error.rs (sketch)

#[derive(Error, Debug)]
pub enum SpindleError {
    #[error("rule not found: {0}")]
    RuleNotFound(String),

    #[error("invalid literal: {0}")]
    InvalidLiteral(String),

    #[error("theory error: {0}")]
    TheoryError(String),

    #[error("reasoning error: {0}")]
    ReasoningError(String),

    #[error("validation error: {message}")]
    Validation { message: String },

    #[error(transparent)]
    Parse(#[from] spindle_parser::ParseError),
}

impl SpindleError {
    /// Returns the stable error code for this variant.
    pub fn code(&self) -> &'static str { /* ... */ }

    /// Returns the error category for taxonomy mapping.
    pub fn category(&self) -> ErrorCategory { /* ... */ }
}
```

### 8.2. Error Propagation

Error propagation uses the `?` operator and `From` trait:

```
parse_spl(input)?           // Result<Theory, ParseError>
    -> prepare(theory)?     // Result<PipelineResult, SpindleError>
                            //   (ParseError auto-converted via #[from])
    -> render(err)          // at CLI boundary: SpindleError -> ProblemDetails
```

The `#[from]` attribute on `SpindleError::Parse` generates `impl From<ParseError> for SpindleError`, enabling `?` to propagate parser errors through the pipeline. The `#[error(transparent)]` attribute delegates `Display` and `source()` to the inner `ParseError`, preserving the error chain.

### 8.3. Presentation Types (new, in spindle-cli)

```rust
// spindle-cli/src/error_report.rs (sketch)

/// RFC 9457 Problem Details, used for rendered error output only.
/// Not used for internal error propagation.
#[derive(Debug, Clone, Serialize)]
pub struct ProblemDetails {
    /// URI reference identifying the problem type.
    /// Default: `"about:blank"` when absent.
    /// Format: `tag:spindle.dev,2026:error:{CODE}`
    #[serde(rename = "type")]
    pub problem_type: String,

    /// Short human-readable summary. Constant per problem type
    /// (RFC 9457: SHOULD NOT change between occurrences).
    pub title: String,

    /// Human-readable explanation specific to this occurrence.
    /// Consumers SHOULD NOT parse this for programmatic information
    /// (RFC 9457 Section 3.1.4). Machine-readable data belongs in
    /// extension members.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,

    /// URI reference identifying this specific occurrence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,

    /// Extension members. Names MUST start with a letter,
    /// contain only ALPHA/DIGIT/underscore, and be at least
    /// three characters (RFC 9457 Section 3.1).
    #[serde(flatten)]
    pub extensions: ProblemExtensions,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProblemExtensions {
    /// CLI exit code. Not the RFC 9457 `status` field.
    pub exit_code: u8,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
}

/// Wraps ProblemDetails with diagnostics for the full JSON envelope.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorReport {
    pub schema_version: &'static str,
    pub diagnostics: Vec<Diagnostic>,
    pub error: ErrorEnvelope,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorEnvelope {
    pub code: String,
    pub message: String,
    pub details: ErrorDetails,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorDetails {
    pub problem: ProblemDetails,
}

/// A non-fatal diagnostic. Does not cause a non-zero exit code.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ProblemDetails {
    /// Implements `std::fmt::Display` for human-readable CLI output.
    /// Format: `Error: {title}` with optional location and hint lines.
    pub fn render_human(&self) -> String { /* ... */ }
}
```

### 8.4. Boundary Conversions

`From` implementations at the CLI/WASM boundary convert library errors into presentation types:

```rust
impl From<&SpindleError> for ProblemDetails {
    fn from(err: &SpindleError) -> Self {
        let code = err.code();
        let category = err.category();
        ProblemDetails {
            problem_type: format!("tag:spindle.dev,2026:error:{code}"),
            title: category.default_title().to_string(),
            detail: Some(err.to_string()),
            instance: None,
            extensions: ProblemExtensions {
                exit_code: category.exit_code(),
                line: extract_line(err),
                column: extract_column(err),
                hint: category.default_hint(),
                source_name: None,
            },
        }
    }
}
```

### 8.5. WASM Boundary

The WASM crate converts errors to `JsError` via `ProblemDetails`:

```rust
// spindle-wasm/src/lib.rs
fn to_js_error(err: SpindleError) -> JsError {
    let pd = ProblemDetails::from(&err);
    JsError::new(&serde_json::to_string(&pd).unwrap_or_else(|_| err.to_string()))
}
```

## 9. Contracts

CON-101: Core Error Interface

Endpoint/Interface: `spindle_core::error::SpindleError` and `spindle_parser::error::ParseError`
Pre-conditions:

- An error condition has been detected during parsing, validation, or reasoning.

Post-conditions:

- A typed `Err` value is returned to the caller via `Result`.
- The error implements `std::error::Error` with a correct `source()` chain.
- `code()` and `category()` methods provide stable classification metadata.

Implements:

- REQ-102
- REQ-105
- REQ-109

Verified by:

- TEST-101
- TEST-106

CON-102: Presentation Error Interface

Endpoint/Interface: `spindle_cli::error_report::ProblemDetails` and `ErrorReport`
Pre-conditions:

- A `SpindleError` or `ParseError` has been received at the presentation boundary.

Post-conditions:

- A `ProblemDetails` instance is produced with stable `type`, `title`, and extension fields.
- `detail` contains only human-readable prose, not structured data (per RFC 9457 Section 3.1.4).

Error model:

- `ProblemDetails` fields:
  - `type` (string, required): `tag:spindle.dev,2026:error:{CODE}`. Default `"about:blank"` when absent, per RFC 9457.
  - `title` (string, required): short human-readable summary, constant per problem type.
  - `detail` (string, optional): human-readable explanation for this occurrence. Consumers SHOULD NOT parse this for programmatic information; use extension members instead.
  - `instance` (string, optional): URI reference identifying this specific occurrence (trace ID).
  - Extension members (per RFC 9457 Section 3.1 naming rules: start with ALPHA, contain ALPHA/DIGIT/underscore, minimum three characters recommended):
    - `exit_code` (integer, required): CLI exit code per the contract.
    - `line` (integer, optional): source line number.
    - `column` (integer, optional): source column number.
    - `hint` (string, optional): actionable suggestion for the user.
    - `source_name` (string, optional): input source identifier (e.g., filename, `"stdin"`).

Note: The RFC 9457 `status` field is intentionally omitted. It is defined as an HTTP status code and does not apply to CLI tooling. CLI exit codes are carried in the `exit_code` extension member (see ADR-105).

Implements:

- REQ-101
- REQ-104
- REQ-106
- REQ-107
- REQ-110

Verified by:

- TEST-101
- TEST-103

CON-103: CLI JSON Error Envelope (Compatibility Contract)

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
        "type": "tag:spindle.dev,2026:error:DFL_PARSE_ERROR",
        "title": "DFL parse error",
        "detail": "Unexpected token near '=>'.",
        "instance": "trace-01HXYZ",
        "exit_code": 2,
        "line": 5,
        "column": 13,
        "hint": "Check the rule syntax near the arrow.",
        "source_name": "stdin"
      }
    }
  }
}
```

Diagnostics-only example (exit code 0):

```json
{
  "schema_version": "spindle.result.v1",
  "diagnostics": [
    {
      "code": "RESULT_TRUNCATED",
      "message": "Output truncated to 1000 results.",
      "detail": "Use --limit to increase the result cap."
    }
  ],
  "result": { "...": "..." }
}
```

Implements:

- REQ-103
- REQ-104
- REQ-108

Verified by:

- TEST-102
- TEST-104
- TEST-105

## 10. Error Taxonomy

### 10.1. Categories

The Error module defines a stable taxonomy with explicit exit code mappings:

| Category | Exit Code | Description |
|---|---|---|
| `PARSE_ERROR` | 2 | Input parsing failures (DFL/SPL lexer or parser errors). |
| `VALIDATION_ERROR` | 2 | Structural or semantic validation failures in the pipeline. |
| `EXECUTION_ERROR` | 3 | Reasoning or query execution failures. |
| `RESOURCE_LIMIT` | 4 | Timeouts, size limits, or truncation errors. |
| `INTERNAL_ERROR` | 4 | Unexpected failures with no safe recovery path. |

Each taxonomy entry maps to:

- A stable `exit_code` extension value (2, 3, or 4).
- A stable `code` for the error envelope (e.g., `DFL_PARSE_ERROR`).
- A `type` URI (e.g., `tag:spindle.dev,2026:error:DFL_PARSE_ERROR`).
- Default `title` and `detail` templates.

### 10.2. Mapping from Existing Error Variants

| Existing Variant | Category | Code | Exit Code |
|---|---|---|---|
| `ParseError::LexerError` | `PARSE_ERROR` | `DFL_LEXER_ERROR` / `SPL_LEXER_ERROR` | 2 |
| `ParseError::ParserError` | `PARSE_ERROR` | `DFL_PARSE_ERROR` / `SPL_PARSE_ERROR` | 2 |
| `ParseError::UnexpectedToken` | `PARSE_ERROR` | `UNEXPECTED_TOKEN` | 2 |
| `ParseError::IoError` | `INTERNAL_ERROR` | `IO_ERROR` | 4 |
| `SpindleError::RuleNotFound` | `EXECUTION_ERROR` | `RULE_NOT_FOUND` | 3 |
| `SpindleError::InvalidLiteral` | `VALIDATION_ERROR` | `INVALID_LITERAL` | 2 |
| `SpindleError::TheoryError` | `VALIDATION_ERROR` | `THEORY_ERROR` | 2 |
| `SpindleError::ReasoningError` | `EXECUTION_ERROR` | `REASONING_ERROR` | 3 |
| `SpindleError::Validation` | `VALIDATION_ERROR` | `VALIDATION_ERROR` | 2 |
| `SpindleError::Parse` | (delegates to inner `ParseError`) | (delegates) | (delegates) |

### 10.3. Stable Error Codes

Error codes are `SCREAMING_SNAKE_CASE` strings. They MUST NOT change across patch versions. Adding a new code requires a minor version update to this specification.

## 11. Rendering Rules

Human-readable CLI errors:

- Format: `Error: {title}` followed by optional `at line {line}, column {column}` and a `Hint: {hint}` line.
- `detail` is shown only when it provides actionable guidance.
- Sensitive details are redacted unless `--debug-errors` is enabled.
- When `--debug-errors` is enabled, the full `source()` error chain is printed.

JSON errors:

- Always include `diagnostics` (may be empty array).
- `error.details.problem` is required when `error` is present.
- `exit_code` extension reflects the CLI exit code.
- The RFC 9457 `status` field is not emitted (see ADR-105).

Diagnostics:

- Remain non-fatal; do not populate `error` when exit code is `0`.
- Must use stable diagnostic codes from the contract.
- The `Diagnostic` type includes `code`, `message`, and optional `detail`.

## 12. Migration Strategy

The migration preserves all existing public API signatures:

1. **Phase 1 — Extend core error types**: Add `code()` and `category()` methods to `SpindleError` and `ParseError`. Add `#[from] ParseError` variant to `SpindleError` for unified propagation. No public API changes.

2. **Phase 2 — Add presentation types**: Introduce `ProblemDetails`, `ErrorReport`, and `Diagnostic` in `spindle-cli`. Implement `From<&SpindleError> for ProblemDetails` and `From<&ParseError> for ProblemDetails`.

3. **Phase 3 — Wire CLI rendering**: Replace ad-hoc `eprintln!` error handling in `spindle-cli/src/main.rs` with `ErrorReport` rendering. JSON mode embeds `ProblemDetails` in the existing envelope.

4. **Phase 4 — Wire WASM rendering**: Replace `JsError::new(&e.to_string())` calls with `ProblemDetails`-based serialization.

## 13. Testing Strategy

TEST-101: `ProblemDetails` construction from each error variant preserves required fields, extension data, and stable codes.
TEST-102: CLI JSON error output contains the contract envelope plus embedded Problem Details with correct field names.
TEST-103: Error rendering redacts sensitive data unless `--debug-errors` is enabled.
TEST-104: Error taxonomy maps to exit codes and stable codes as specified in Section 10.2.
TEST-105: Diagnostics-only cases (e.g., truncation) emit no `error` object and exit with code 0.
TEST-106: `source()` chain is preserved through `From` conversions (e.g., `ParseError` -> `SpindleError::Parse` -> `ProblemDetails`).
TEST-107: `From<&SpindleError> for ProblemDetails` and `From<&ParseError> for ProblemDetails` produce valid RFC 9457-compatible output.
TEST-108: Extension member names conform to RFC 9457 naming rules (ALPHA start, ALPHA/DIGIT/underscore body).

## 14. Traceability

Trace links:

- REQ-101 → CON-102 → TEST-101
- REQ-102 → CON-101 → TEST-107
- REQ-103 → CON-103 → TEST-102
- REQ-104 → CON-103 → TEST-104
- REQ-105 → CON-101, CON-102 → TEST-104
- REQ-106 → CON-102 → TEST-101
- REQ-107 → CON-102 → TEST-103
- REQ-108 → CON-103 → TEST-105
- REQ-109 → CON-101 → TEST-106
- REQ-110 → CON-102 → TEST-107

---

## Document History

| Version | Date | Author | Changes |
|---|---|---|---|
| 0.1.0 | 2026-02-09 | Codex (AI agent) | Initial draft |
| 0.2.0 | 2026-02-10 | Claude (AI agent) | RFC 9457 alignment, Rust error handling architecture, migration strategy, taxonomy mapping |

---

**END OF SPECIFICATION**
