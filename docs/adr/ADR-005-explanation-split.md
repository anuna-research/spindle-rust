# ADR-005: Explanation Types vs Formatters Separation

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-02-11 |
| **Deciders** | Spindle core maintainers |
| **Scope** | `crates/spindle-core/src/explanation.rs` |

## 1. Context and Problem Statement

`explanation.rs` is currently a 2,226-line monolith that conflates two distinct responsibilities:

1. **Data types** -- the proof-tree model (`ProofNode`, `ProofStep`, `BlockedProof`, `ConflictResolution`, `Explanation`, `Annotations`, `DerivationType`, `BlockReason`, `ResolutionType`).
2. **Rendering** -- four output formats (`to_natural_language`, `to_json`, `to_jsonld`, `to_dot`) plus their recursive helpers (`proof_node_to_natural_language`, `proof_node_to_json`, `blocked_proof_to_json`, `conflict_to_json`, `proof_node_to_jsonld`, `blocked_proof_to_jsonld`, `conflict_to_jsonld`, `escape_dot_label`, `render_proof_node_to_dot`, `rule_type_name`).

The file contains 8 `impl` blocks and 12 free functions. Of the free functions, 10 exist solely for rendering. The `Explanation` impl block alone holds 4 rendering methods (`to_natural_language`, `to_json`, `to_jsonld`, `to_dot`) alongside 4 data-construction methods (`new`, `with_proof`, `with_blocked`, `with_conflicts`) and 1 private helper (`conclusion_type_explanation`).

### Problems this causes

| Problem | Impact |
|---------|--------|
| Adding a new output format (e.g., YAML, HTML, Markdown) requires editing the core data types file | Violates Open/Closed Principle |
| `serde_json` is a hard dependency of the types module even though only JSON/JSON-LD formatters need it | Unnecessary coupling; bloats WASM binary |
| Rendering tests (50+ assertions on string output) are interleaved with structural unit tests | Slow test feedback; hard to add golden-file tests |
| Downstream consumers who only need the data types (e.g., `spindle-wasm`) pull in DOT-graph rendering code | Dead code in size-constrained targets |
| CLI (`explain.rs`) calls `explanation.to_json()` and `explanation.to_natural_language()` directly on the struct -- no way to inject a custom formatter | No extensibility for users |

### Current call sites

The rendering methods are consumed in two places outside the module:

- **`crates/spindle-cli/src/cli/commands/explain.rs`** -- calls `explanation.to_json()` for `--json` mode and `explanation.to_natural_language()` for text mode.
- **`crates/spindle-core/benches/reasoning.rs`** -- benchmarks `to_natural_language`, `to_json`, and `to_dot`.

All other uses are internal tests within `explanation.rs` itself.

## 2. Decision

Split `explanation.rs` into a module directory with the following structure:

```
crates/spindle-core/src/explanation/
    mod.rs              -- re-exports, explain() function, explain_inner()
    types.rs            -- pure data types (no rendering)
    format/
        mod.rs          -- ExplanationFormatter trait + registry
        natural_language.rs
        json.rs
        jsonld.rs
        dot.rs
```

### 2.1 Types module (`explanation/types.rs`)

All data types move here. They gain `#[derive(Clone, Debug, serde::Serialize)]` but lose every rendering method. Construction helpers (`new`, `with_*` builders) stay on the types.

```rust
// explanation/types.rs

use std::collections::HashMap;
use std::fmt;

use serde::Serialize;

use crate::conclusion::ConclusionType;
use crate::literal::Literal;
use crate::rule::RuleType;

// ---------------------------------------------------------------------------
// Annotations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
pub struct Annotations {
    pub id: Option<String>,
    pub entries: HashMap<String, String>,
}

impl Annotations {
    pub fn new() -> Self { Self::default() }

    pub fn with_entries(entries: Vec<(&str, &str)>) -> Self {
        Self {
            id: None,
            entries: entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.id.is_none() && self.entries.is_empty()
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(|s| s.as_str())
    }

    pub fn get_any(&self, keys: &[&str]) -> Option<&str> {
        keys.iter().find_map(|k| self.get(k))
    }

    pub fn description(&self) -> Option<&str> {
        self.get_any(&["description", "dc:description", "rdfs:comment"])
    }

    pub fn source(&self) -> Option<&str> {
        self.get_any(&["source", "dc:source", "prov:wasAttributedTo"])
    }

    pub fn confidence(&self) -> Option<&str> {
        self.get_any(&["confidence", "spindle:confidence"])
    }
}

// ---------------------------------------------------------------------------
// Derivation / Block / Resolution enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DerivationType {
    Definite,
    Defeasible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BlockReason {
    Superiority,
    Defeater,
    Conflict,
    BodyUnprovable,
}

impl fmt::Display for BlockReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Superiority   => write!(f, "superiority"),
            Self::Defeater      => write!(f, "defeater"),
            Self::Conflict      => write!(f, "conflict"),
            Self::BodyUnprovable => write!(f, "body unprovable"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ResolutionType {
    Superiority,
    DefinitePriority,
    TeamDefeat,
}

impl fmt::Display for ResolutionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Superiority      => write!(f, "superiority"),
            Self::DefinitePriority => write!(f, "definite priority"),
            Self::TeamDefeat       => write!(f, "team defeat"),
        }
    }
}

// ---------------------------------------------------------------------------
// Core data structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ProofStep {
    pub rule_label: String,
    pub rule_type: RuleType,
    pub rule_text: String,
    pub body_proofs: Vec<ProofNode>,
    pub annotations: Annotations,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProofNode {
    pub literal: Literal,
    pub derivation_type: DerivationType,
    pub proof_step: Option<ProofStep>,
    pub blocked_alternatives: Vec<BlockedProof>,
    pub conflicts_resolved: Vec<ConflictResolution>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlockedProof {
    pub literal: Literal,
    pub rule_label: String,
    pub rule_text: String,
    pub reason: BlockReason,
    pub blocking_rule: Option<String>,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConflictResolution {
    pub winning_rule: String,
    pub losing_rule: String,
    pub resolution_type: ResolutionType,
    pub superiority_label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Explanation {
    pub conclusion_type: ConclusionType,
    pub literal: Literal,
    pub proof_tree: Option<ProofNode>,
    pub blocked_alternatives: Vec<BlockedProof>,
    pub conflicts_resolved: Vec<ConflictResolution>,
}

// Builder methods stay on the types -- they are data construction, not rendering.
impl ProofStep {
    pub fn new(
        rule_label: impl Into<String>,
        rule_type: RuleType,
        rule_text: impl Into<String>,
    ) -> Self { /* ... */ }

    pub fn with_body_proofs(mut self, proofs: Vec<ProofNode>) -> Self { /* ... */ }
    pub fn with_annotations(mut self, annotations: Annotations) -> Self { /* ... */ }
}

impl ProofNode {
    pub fn new(literal: Literal, derivation_type: DerivationType) -> Self { /* ... */ }
    pub fn with_proof_step(mut self, step: ProofStep) -> Self { /* ... */ }
    pub fn with_blocked(mut self, blocked: Vec<BlockedProof>) -> Self { /* ... */ }
    pub fn with_conflicts(mut self, conflicts: Vec<ConflictResolution>) -> Self { /* ... */ }
}

impl BlockedProof {
    pub fn new(/* ... */) -> Self { /* ... */ }
    pub fn with_blocking_rule(mut self, rule: impl Into<String>) -> Self { /* ... */ }
}

impl ConflictResolution {
    pub fn new(/* ... */) -> Self { /* ... */ }
    pub fn with_superiority(mut self, label: impl Into<String>) -> Self { /* ... */ }
}

impl Explanation {
    pub fn new(conclusion_type: ConclusionType, literal: Literal) -> Self { /* ... */ }
    pub fn with_proof(mut self, proof: ProofNode) -> Self { /* ... */ }
    pub fn with_blocked(mut self, blocked: Vec<BlockedProof>) -> Self { /* ... */ }
    pub fn with_conflicts(mut self, conflicts: Vec<ConflictResolution>) -> Self { /* ... */ }
}
```

Key decisions on the types:

- Every struct derives `Clone + Debug + Serialize`. This makes them sufficient for custom serialization without requiring any rendering code.
- No `serde::Deserialize` yet -- there is no current need for round-tripping explanations from serialized form. It can be added later if needed.
- `fmt::Display` impls on enums (`BlockReason`, `ResolutionType`) stay on the types. They are intrinsic string representations, not format-specific rendering.
- The `Explanation` struct no longer has `to_natural_language()`, `to_json()`, `to_jsonld()`, or `to_dot()` methods.

### 2.2 Formatter trait (`explanation/format/mod.rs`)

```rust
// explanation/format/mod.rs

pub mod dot;
pub mod json;
pub mod jsonld;
pub mod natural_language;

use super::types::Explanation;

/// A stateless, infallible formatter that converts an Explanation into a
/// string representation.
///
/// # Design constraints
///
/// - **Stateless**: Formatters carry no mutable state. Configuration (e.g.,
///   indentation width, color palette) is provided at construction time and
///   stored in immutable fields.
/// - **Infallible**: `format()` returns `String`, never `Result`. Rendering
///   an in-memory proof tree cannot fail; any errors belong upstream in the
///   reasoning pipeline.
/// - **Borrowed input**: The formatter borrows the Explanation, allowing
///   the caller to format the same explanation in multiple formats without
///   cloning.
pub trait ExplanationFormatter {
    /// Render the explanation to a string.
    fn format(&self, explanation: &Explanation) -> String;
}
```

### 2.3 Concrete formatters

Each formatter lives in its own file and implements `ExplanationFormatter`.

#### Natural language (`explanation/format/natural_language.rs`)

```rust
use super::ExplanationFormatter;
use crate::explanation::types::*;
use crate::rule::RuleType;

/// Renders explanations as human-readable English text.
#[derive(Debug, Clone, Default)]
pub struct NaturalLanguageFormatter {
    /// Indentation string (default: two spaces).
    pub indent: String,
}

impl NaturalLanguageFormatter {
    pub fn new() -> Self {
        Self {
            indent: "  ".to_string(),
        }
    }
}

impl ExplanationFormatter for NaturalLanguageFormatter {
    fn format(&self, explanation: &Explanation) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&format!(
            "Explanation for {} {}\n",
            explanation.conclusion_type, explanation.literal
        ));
        output.push_str(&format!(
            "{}\n\n",
            conclusion_type_explanation(explanation.conclusion_type)
        ));

        // Proof tree
        if let Some(ref proof) = explanation.proof_tree {
            output.push_str("Derivation:\n");
            render_proof_node(proof, 1, &self.indent, &mut output);
        } else if explanation.conclusion_type.is_positive() {
            output.push_str("No derivation found.\n");
        }

        // Blocked alternatives
        if !explanation.blocked_alternatives.is_empty() {
            output.push_str("\nBlocked Alternatives:\n");
            for (i, blocked) in explanation.blocked_alternatives.iter().enumerate() {
                output.push_str(&format!(
                    "  {}. Rule '{}' was blocked due to {}: {}\n",
                    i + 1, blocked.rule_label, blocked.reason, blocked.explanation
                ));
            }
        }

        // Conflict resolutions
        if !explanation.conflicts_resolved.is_empty() {
            output.push_str("\nConflict Resolutions:\n");
            for (i, conflict) in explanation.conflicts_resolved.iter().enumerate() {
                output.push_str(&format!(
                    "  {}. '{}' defeated '{}' via {}\n",
                    i + 1,
                    conflict.winning_rule,
                    conflict.losing_rule,
                    conflict.resolution_type
                ));
            }
        }

        output
    }
}

// Private helpers (moved from explanation.rs free functions)
fn render_proof_node(
    node: &ProofNode,
    num: usize,
    indent: &str,
    output: &mut String,
) {
    // ... same logic as current proof_node_to_natural_language
}

fn conclusion_type_explanation(ct: ConclusionType) -> &'static str {
    // ... same logic as current Explanation::conclusion_type_explanation
}

fn rule_type_name(rt: RuleType) -> &'static str {
    // ... same logic as current rule_type_name
}
```

#### JSON (`explanation/format/json.rs`)

```rust
use super::ExplanationFormatter;
use crate::explanation::types::*;

/// Renders explanations as a flat JSON structure.
#[derive(Debug, Clone, Default)]
pub struct JsonFormatter;

impl ExplanationFormatter for JsonFormatter {
    fn format(&self, explanation: &Explanation) -> String {
        let value = self.to_value(explanation);
        serde_json::to_string_pretty(&value)
            .expect("Explanation JSON serialization cannot fail")
    }
}

impl JsonFormatter {
    /// Return the serde_json::Value for callers that need structured data
    /// (e.g., the CLI embedding it in a larger JSON envelope).
    pub fn to_value(&self, explanation: &Explanation) -> serde_json::Value {
        serde_json::json!({
            "conclusion_type": explanation.conclusion_type.symbol(),
            "literal": explanation.literal.to_string(),
            "proof_tree": explanation.proof_tree.as_ref().map(|p| proof_node_to_json(p)),
            "blocked_alternatives": explanation.blocked_alternatives.iter()
                .map(blocked_proof_to_json).collect::<Vec<_>>(),
            "conflicts_resolved": explanation.conflicts_resolved.iter()
                .map(conflict_to_json).collect::<Vec<_>>(),
        })
    }
}

// Private helpers (moved from explanation.rs)
fn proof_node_to_json(node: &ProofNode) -> serde_json::Value { /* ... */ }
fn blocked_proof_to_json(blocked: &BlockedProof) -> serde_json::Value { /* ... */ }
fn conflict_to_json(conflict: &ConflictResolution) -> serde_json::Value { /* ... */ }
```

#### JSON-LD (`explanation/format/jsonld.rs`)

```rust
use super::ExplanationFormatter;
use crate::explanation::types::*;

/// Renders explanations as JSON-LD with semantic annotations.
#[derive(Debug, Clone, Default)]
pub struct JsonLdFormatter;

impl ExplanationFormatter for JsonLdFormatter {
    fn format(&self, explanation: &Explanation) -> String {
        let value = self.to_value(explanation);
        serde_json::to_string_pretty(&value)
            .expect("Explanation JSON-LD serialization cannot fail")
    }
}

impl JsonLdFormatter {
    pub fn to_value(&self, explanation: &Explanation) -> serde_json::Value {
        // ... same logic as current Explanation::to_jsonld()
    }
}

// Private helpers
fn proof_node_to_jsonld(node: &ProofNode) -> serde_json::Value { /* ... */ }
fn blocked_proof_to_jsonld(blocked: &BlockedProof) -> serde_json::Value { /* ... */ }
fn conflict_to_jsonld(conflict: &ConflictResolution) -> serde_json::Value { /* ... */ }
```

#### DOT (`explanation/format/dot.rs`)

```rust
use super::ExplanationFormatter;
use crate::explanation::types::*;

/// Renders explanations as Graphviz DOT graphs.
#[derive(Debug, Clone)]
pub struct DotFormatter {
    /// Graph direction: "BT" (bottom-to-top) or "TB" (top-to-bottom).
    pub rank_dir: String,
    /// Font for nodes and edges.
    pub font: String,
}

impl Default for DotFormatter {
    fn default() -> Self {
        Self {
            rank_dir: "BT".to_string(),
            font: "Helvetica".to_string(),
        }
    }
}

impl ExplanationFormatter for DotFormatter {
    fn format(&self, explanation: &Explanation) -> String {
        // ... same logic as current Explanation::to_dot(), using self.rank_dir
        //     and self.font instead of hardcoded values
    }
}

// Private helpers
fn escape_dot_label(s: &str) -> String { /* ... */ }
fn render_proof_node(
    node: &ProofNode,
    output: &mut String,
    counter: &mut usize,
) -> usize { /* ... */ }
```

### 2.4 Module root (`explanation/mod.rs`)

```rust
//! Explanation System for Spindle
//!
//! Data types live in `types`. Output formatters live in `format`.

pub mod format;
pub mod types;

// Re-export everything at the same path as before for backward compatibility.
pub use types::*;

// The explain() function and explain_inner() stay here -- they are
// reasoning logic, not rendering.
use std::collections::HashSet;
use crate::error::Result;
use crate::literal::Literal;

pub fn explain(
    theory: &crate::theory::Theory,
    literal: &Literal,
) -> Result<Option<Explanation>> {
    let mut visited = HashSet::new();
    explain_inner(theory, literal, &mut visited)
}

fn explain_inner(/* ... */) -> Result<Option<Explanation>> {
    // ... unchanged
}
```

### 2.5 Backward-compatible convenience methods

To avoid a flag-day migration, the old method names are kept as thin wrappers during a deprecation period:

```rust
// In explanation/types.rs (or a separate compat module)

impl Explanation {
    #[deprecated(since = "0.next", note = "Use NaturalLanguageFormatter::format() instead")]
    pub fn to_natural_language(&self) -> String {
        format::natural_language::NaturalLanguageFormatter::new().format(self)
    }

    #[deprecated(since = "0.next", note = "Use JsonFormatter::to_value() instead")]
    pub fn to_json(&self) -> serde_json::Value {
        format::json::JsonFormatter.to_value(self)
    }

    #[deprecated(since = "0.next", note = "Use JsonLdFormatter::to_value() instead")]
    pub fn to_jsonld(&self) -> serde_json::Value {
        format::jsonld::JsonLdFormatter.to_value(self)
    }

    #[deprecated(since = "0.next", note = "Use DotFormatter::format() instead")]
    pub fn to_dot(&self) -> String {
        format::dot::DotFormatter::default().format(self)
    }
}
```

These wrappers are removed in the next minor release after all call sites migrate.

### 2.6 Migration of call sites

The CLI explain command changes from:

```rust
// Before
explanation.to_json()
explanation.to_natural_language()
```

to:

```rust
// After
use spindle_core::explanation::format::json::JsonFormatter;
use spindle_core::explanation::format::natural_language::NaturalLanguageFormatter;
use spindle_core::explanation::format::ExplanationFormatter;

// In --json mode:
JsonFormatter.to_value(&explanation)

// In text mode:
NaturalLanguageFormatter::new().format(&explanation)
```

The benchmarks change similarly:

```rust
use spindle_core::explanation::format::{
    ExplanationFormatter,
    natural_language::NaturalLanguageFormatter,
    json::JsonFormatter,
    dot::DotFormatter,
};

group.bench_function("natural_language", |b| {
    let fmt = NaturalLanguageFormatter::new();
    b.iter(|| black_box(fmt.format(&exp)))
});
group.bench_function("json", |b| {
    let fmt = JsonFormatter;
    b.iter(|| black_box(fmt.format(&exp)))
});
group.bench_function("dot", |b| {
    let fmt = DotFormatter::default();
    b.iter(|| black_box(fmt.format(&exp)))
});
```

## 3. Testing Strategy

### 3.1 Types tests

Structural unit tests (builder patterns, field access, enum Display impls) move to `explanation/types.rs` under `#[cfg(test)] mod tests`. These are fast and deterministic.

### 3.2 Golden-file tests for formatters

Each formatter gets a `tests/golden/` directory with `.expected` snapshot files:

```
crates/spindle-core/tests/golden/explanation/
    natural_language/
        simple_fact.expected
        defeasible_chain.expected
        blocked_alternatives.expected
        conflict_resolution.expected
        full_penguin.expected
    json/
        simple_fact.expected.json
        defeasible_chain.expected.json
    jsonld/
        simple_fact.expected.json
        with_annotations.expected.json
    dot/
        simple_fact.expected.dot
        complex_tree.expected.dot
```

A shared test helper constructs canonical `Explanation` fixtures:

```rust
// crates/spindle-core/tests/explanation_fixtures.rs

use spindle_core::explanation::types::*;
use spindle_core::conclusion::ConclusionType;
use spindle_core::literal::Literal;
use spindle_core::rule::RuleType;

/// The classic penguin example with proof tree, blocked alternative, and
/// conflict resolution.
pub fn penguin_full() -> Explanation {
    let penguin_step = ProofStep::new("f1", RuleType::Fact, ">> penguin");
    let penguin_proof =
        ProofNode::new(Literal::simple("penguin"), DerivationType::Definite)
            .with_proof_step(penguin_step);

    let bird_step = ProofStep::new("s1", RuleType::Strict, "penguin -> bird")
        .with_body_proofs(vec![penguin_proof]);
    let bird_proof =
        ProofNode::new(Literal::simple("bird"), DerivationType::Definite)
            .with_proof_step(bird_step);

    let not_flies_step =
        ProofStep::new("r2", RuleType::Defeasible, "penguin => ~flies")
            .with_body_proofs(vec![bird_proof]);
    let not_flies_proof =
        ProofNode::new(Literal::negated("flies"), DerivationType::Defeasible)
            .with_proof_step(not_flies_step);

    let blocked = BlockedProof::new(
        Literal::simple("flies"),
        "r1",
        BlockReason::Superiority,
        "r2 > r1 via superiority",
    );

    let conflict = ConflictResolution::new("r2", "r1", ResolutionType::Superiority)
        .with_superiority("s1");

    Explanation::new(
        ConclusionType::DefeasiblyProvable,
        Literal::negated("flies"),
    )
    .with_proof(not_flies_proof)
    .with_blocked(vec![blocked])
    .with_conflicts(vec![conflict])
}
```

Golden-file test pattern:

```rust
#[test]
fn test_natural_language_penguin_full() {
    let explanation = explanation_fixtures::penguin_full();
    let formatter = NaturalLanguageFormatter::new();
    let actual = formatter.format(&explanation);

    let expected = include_str!("golden/explanation/natural_language/full_penguin.expected");
    assert_eq!(actual, expected, "Golden file mismatch. Run with UPDATE_GOLDEN=1 to update.");
}
```

With an optional `UPDATE_GOLDEN=1` env var that writes the actual output back to disk when snapshots need updating. This is a common pattern in Rust projects (similar to `insta` but lighter-weight).

### 3.3 Formatter trait contract tests

A generic test suite that any `ExplanationFormatter` implementation must pass:

```rust
fn assert_formatter_contract(formatter: &dyn ExplanationFormatter) {
    // 1. Empty explanation (no proof tree, no blocked, no conflicts)
    let empty = Explanation::new(
        ConclusionType::DefeasiblyNotProvable,
        Literal::simple("p"),
    );
    let output = formatter.format(&empty);
    assert!(!output.is_empty(), "Formatter must produce non-empty output");

    // 2. Output must contain the literal name
    assert!(output.contains("p"), "Output must mention the literal");

    // 3. Deterministic: formatting twice yields identical output
    let output2 = formatter.format(&empty);
    assert_eq!(output, output2, "Formatter must be deterministic");
}
```

## 4. Trade-offs

### Advantages

| Advantage | Detail |
|-----------|--------|
| **Single Responsibility** | Types know nothing about rendering; formatters know nothing about reasoning. Each file is under 300 LOC. |
| **Open for extension** | Adding a new format (e.g., Markdown, HTML) requires only a new file implementing `ExplanationFormatter`. No existing code changes. |
| **Testability** | Golden-file tests catch rendering regressions with clear diffs. Type tests run independently and fast. |
| **Conditional compilation** | `serde_json` can be gated behind a `json` feature flag. WASM builds that only need natural-language output avoid pulling in the JSON stack. |
| **Configurability** | `DotFormatter { rank_dir: "TB", font: "Courier" }` replaces hardcoded values. Users can customize without forking. |
| **Trait-based dispatch** | CLI can accept `Box<dyn ExplanationFormatter>` and dispatch based on `--format` flag without a match cascade. |

### Disadvantages

| Disadvantage | Mitigation |
|--------------|------------|
| **More files** | 6 files instead of 1. Offset by each file being small and focused. IDE navigation via `mod.rs` re-exports. |
| **Import path length** | `explanation::format::json::JsonFormatter` is longer than `explanation.to_json()`. Mitigated by re-exports in `format/mod.rs` and the deprecation wrappers during transition. |
| **Backward compatibility** | Deprecated wrappers on `Explanation` keep old call sites compiling. One-release deprecation window. |
| **Golden file maintenance** | Snapshot files must be updated when formatting changes intentionally. `UPDATE_GOLDEN=1` env var automates this. |
| **Trait object overhead** | `dyn ExplanationFormatter` involves vtable dispatch. For explanation formatting (which is not on the hot path), this overhead is negligible. Static dispatch via generics is also available. |

## 5. Consequences

### Immediate (this PR)

1. Create `explanation/` module directory with `mod.rs`, `types.rs`, and `format/` subdirectory.
2. Move all struct/enum definitions and their builder `impl` blocks into `types.rs`.
3. Move each formatter's logic into its own file under `format/`.
4. Add `#[deprecated]` wrappers on `Explanation` for `to_natural_language`, `to_json`, `to_jsonld`, `to_dot`.
5. Existing tests pass without modification (deprecated wrappers forward to new formatters).
6. Add golden-file test scaffolding for at least the penguin example in each format.

### Follow-up (subsequent PRs)

1. Migrate CLI and bench call sites to use formatter structs directly; remove deprecated wrappers.
2. Gate `json` and `jsonld` formatters behind a `json` cargo feature (default-on).
3. Gate `dot` formatter behind a `dot` cargo feature (default-on).
4. Add `MarkdownFormatter` for documentation generation use cases.
5. Expose `ExplanationFormatter` in `spindle-wasm` so JavaScript consumers can register custom formatters.

### Invariants preserved

- `pub use types::*` in `explanation/mod.rs` means all existing import paths (`use spindle_core::explanation::Explanation`, etc.) continue to resolve.
- `pub fn explain()` stays at `spindle_core::explanation::explain` -- the public API for explanation generation does not move.
- The `Explanation` struct is still `Clone + Debug` (and now additionally `Serialize`).
- All 47 existing tests in the `explanation` module continue to pass through the deprecated wrappers.

## 6. Alternatives Considered

### A. Keep rendering methods on `Explanation`, extract only helpers

Move the recursive free functions (`proof_node_to_json`, etc.) into sub-modules but keep `to_json()` etc. as methods on `Explanation`.

**Rejected** because this does not solve the extensibility problem. Adding a new format still requires editing the `Explanation` impl block.

### B. Use `serde::Serialize` directly for JSON output

Derive `Serialize` on all types and let `serde_json::to_string()` produce the JSON output, eliminating the hand-rolled `to_json()`.

**Partially adopted**: types now derive `Serialize`. However, the current JSON schema (with `conclusion_type` using the symbol like `+d` and custom key naming) differs from what a naive `#[derive(Serialize)]` would produce. The `JsonFormatter` preserves the existing schema for backward compatibility while using `Serialize` internally for potential future simplification.

### C. Visitor pattern instead of trait

Define an `ExplanationVisitor` that walks the proof tree with callbacks for each node type.

**Rejected** as over-engineered for the current four formats. The `ExplanationFormatter` trait with `&Explanation` input is simpler and sufficient. A visitor can be introduced later if formatters need fine-grained tree-walking hooks.

## 7. References

- Current source: `crates/spindle-core/src/explanation.rs` (2,226 LOC)
- CLI consumer: `crates/spindle-cli/src/cli/commands/explain.rs`
- Bench consumer: `crates/spindle-core/benches/reasoning.rs`
- Documentation: `docs/src/guides/explanations.md`
