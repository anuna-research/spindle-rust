# Spindle Contract Crate Design

| Field | Value |
|---|---|
| Document ID | DESIGN-001 |
| Title | `spindle-contract` Shared Transport Types Crate |
| Status | Draft |
| Created | 2026-02-10 |
| Parent | [IMPL-010](./IMPL-010-spindle-simplification-cleanup.md) |

## 1. Purpose

The `spindle-contract` crate owns the shared transport-level data transfer objects (DTOs) used by both `spindle-cli` and `spindle-wasm` for structured output. This eliminates duplicate type definitions that currently drift independently across presentation crates.

## 2. Problem

The CLI and WASM crates define near-identical output structs independently:

| CLI (`main.rs`) | WASM (`lib.rs`) | Schema |
|---|---|---|
| `ReasonOutput` | `JsReasonOutput` | `spindle.reason.v1` |
| `GroundingStats` | `JsGroundingStats` | (nested) |
| `ConclusionStruct` | `JsConclusionStruct` | (nested) |
| `TheoryStats` | `JsTheoryStats` | (nested) |
| `LiteralStructJson` | *(uses core `LiteralStruct`)* | (nested) |
| `ModeJson` | *(uses core `Mode`)* | (nested) |
| `TemporalJson` | *(uses core `Temporal`)* | (nested) |

Additional CLI-only output types (`QueryOutput`, `ExplainOutput`, `WhyNotOutput`, `RequiresOutput`, `StatsOutput`, `ValidateOutput`, `CapabilitiesOutput`) are candidates for inclusion if WASM later gains matching endpoints.

## 3. Dependency Direction

```
spindle-core  (domain types: Literal, Mode, Temporal, Theory, Conclusion)
     ^
     |
spindle-contract  (transport DTOs: ReasonOutput, error envelope types)
     ^         ^
     |         |
spindle-cli   spindle-wasm
```

- `spindle-contract` depends on `spindle-core` for `Literal`, `Mode`, `Temporal`, `ConclusionType`.
- `spindle-cli` and `spindle-wasm` depend on `spindle-contract` for shared DTOs.
- `spindle-core` does NOT depend on `spindle-contract` (no cycles).
- `spindle-parser` is unaffected.

## 4. Crate Contents

### 4.1. Reason Output Types (Phase C, initial)

```rust
// crates/spindle-contract/src/reason.rs

/// Structured reason output — schema `spindle.reason.v1`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasonOutput {
    pub schema_version: String,
    pub evaluated_at: Option<String>,
    pub grounding: GroundingStats,
    pub conclusions: Vec<ConclusionEntry>,
    pub diagnostics: Vec<DiagnosticEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<TheoryStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingStats {
    pub performed: bool,
    pub had_variables: bool,
    pub instances: usize,
    pub limit_hit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConclusionEntry {
    pub conclusion_type: String,
    pub literal_spl: String,
    pub literal_struct: LiteralStructJson,
    pub positive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TheoryStats {
    pub rule_count: usize,
    pub fact_count: usize,
}
```

### 4.2. Literal Transport Types

```rust
// crates/spindle-contract/src/literal.rs

/// JSON-serializable literal representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiteralStructJson {
    pub mode: ModeJson,
    pub negated: bool,
    pub functor: String,
    pub args: Vec<String>,
    pub temporal: TemporalJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeJson {
    pub name: Option<String>,
    pub negation: bool,
}

/// Maps NegInf/PosInf to null per contract schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalJson {
    pub start: Option<i64>,
    pub end: Option<i64>,
}
```

`From<&Literal>`, `From<&Mode>`, and `From<&Temporal>` conversions live in this crate since it depends on `spindle-core`.

### 4.3. Diagnostic Entry (shared)

```rust
// crates/spindle-contract/src/diagnostic.rs

/// A diagnostic message for output envelopes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticEntry {
    pub severity: String,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}
```

### 4.4. Error Envelope Types (Phase D, future)

After the error module (SPEC-010) lands, the contract crate will also own:

- `ProblemDetails` — RFC 9457 presentation error model.
- `ProblemExtensions` — CLI exit code, location, hint, source context.
- `ErrorReport` / `ErrorEnvelope` / `ErrorDetails` — contract-compliant JSON error wrapper.
- `SourceContext` / `SourceLine` — source context window types.

These are deferred to Phase D to avoid coupling the contract crate to error types that don't exist yet.

## 5. Cargo.toml

```toml
[package]
name = "spindle-contract"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Shared transport types for Spindle CLI and WASM output"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
spindle-core = { path = "../spindle-core" }
```

Add to workspace `Cargo.toml`:

```toml
members = [
    "crates/spindle-core",
    "crates/spindle-parser",
    "crates/spindle-contract",  # new
    "crates/spindle-cli",
    "crates/spindle-wasm",
]
```

## 6. Migration Path

1. Create `crates/spindle-contract/` with the types above.
2. In `spindle-cli`: replace `ReasonOutput`, `GroundingStats`, `ConclusionStruct`, `TheoryStats`, `LiteralStructJson`, `ModeJson`, `TemporalJson` with re-exports from `spindle-contract`.
3. In `spindle-wasm`: replace `JsReasonOutput`, `JsGroundingStats`, `JsConclusionStruct`, `JsTheoryStats` with re-exports from `spindle-contract`.
4. Run `cargo test --workspace` to verify no regressions.
5. Add parity tests in `spindle-contract` that serialize identical inputs through both paths.

## 7. What Stays Out

- **Core domain types** (`Literal`, `Theory`, `Rule`, `Conclusion`) stay in `spindle-core`. The contract crate only holds serialization-oriented transport types.
- **CLI-specific output types** (`QueryOutput`, `ExplainOutput`, etc.) stay in `spindle-cli` until WASM gains matching endpoints.
- **WASM-specific bindings** (`#[wasm_bindgen]` annotations, `JsValue` conversions) stay in `spindle-wasm`.
- **Rendering logic** (`render_human()`, `emit_and_exit()`) stays in presentation crates.

## 8. Naming Convention

The canonical names drop the `Js` prefix and the `Json` suffix used in the current crates:

| Current (CLI) | Current (WASM) | Contract Crate |
|---|---|---|
| `ReasonOutput` | `JsReasonOutput` | `ReasonOutput` |
| `GroundingStats` | `JsGroundingStats` | `GroundingStats` |
| `ConclusionStruct` | `JsConclusionStruct` | `ConclusionEntry` |
| `TheoryStats` | `JsTheoryStats` | `TheoryStats` |
| `LiteralStructJson` | *(core `LiteralStruct`)* | `LiteralStructJson` |
| `ModeJson` | *(core `Mode`)* | `ModeJson` |
| `TemporalJson` | *(core `Temporal`)* | `TemporalJson` |

`ConclusionStruct` is renamed to `ConclusionEntry` to avoid confusion with `spindle_core::conclusion::Conclusion`.
