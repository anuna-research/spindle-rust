# ADR-002: Pipeline as Composable Middleware

| Field       | Value                                      |
|-------------|-------------------------------------------|
| Status      | Proposed                                   |
| Date        | 2026-02-11                                 |
| Deciders    | Spindle maintainers                        |
| Supersedes  | --                                         |
| Relates to  | `crates/spindle-core/src/pipeline.rs`      |

## Context

### Current state

The `prepare()` function in `pipeline.rs` is the single entry point for transforming a
raw `Theory` into a form ready for reasoning. It executes four distinct logical phases
in a hardcoded sequence:

1. **Temporal filtering** -- removes rules/facts inactive at a given `TimePoint`
   (`filter_temporal`).
2. **Validation** -- rejects wildcards in heads and enforces range restriction
   (`validate_wildcards`, `validate_range_restriction`).
3. **Wildcard rewrite** -- replaces `_` with unique `?_wN` variables
   (`rewrite_wildcards`).
4. **Grounding** -- Datalog-style bottom-up variable instantiation
   (`ground_theory_with_limit` from `grounding.rs`).

Configuration is flattened into a single `PrepareOptions` struct that mixes concerns
(temporal reference time, grounding limits, validation toggles, trust policy override).

### Problems

| Problem | Impact |
|---------|--------|
| **No way to insert custom stages.** Downstream consumers (WASM playground, CLI batch mode, language-server protocol) cannot inject steps such as lint passes, macro expansion, or claim normalization without forking `prepare()`. | Limits extensibility. |
| **No way to skip stages.** Disabling grounding requires a boolean flag; disabling temporal filtering requires passing `None`. There is no uniform mechanism. | Configuration is ad-hoc. |
| **No diagnostics accumulator.** Validation aborts on the first error. There is no way to collect warnings (e.g., "grounding limit approached") alongside hard errors. | Poor user experience in editor integrations. |
| **Ordering constraints are implicit.** The fact that wildcards must be rewritten before grounding, and that validation must precede wildcard rewrite, is encoded only by source-code order inside `prepare()`. | Fragile to refactoring. |
| **Monolithic return type.** `PipelineResult` bundles a `GroundingReport` that only makes sense when grounding actually runs, plus a `weighted_conclusions` vec that is always empty (trust-weighted conclusions are computed separately). | Awkward API surface. |

## Decision

Refactor the pipeline into a chain of composable **stages** behind a `PipelineStage`
trait, assembled at call-site via a builder. The default builder reproduces the current
`prepare()` behavior exactly.

### Core trait

```rust
use crate::error::Result;
use crate::theory::Theory;

/// Metadata and diagnostics accumulator threaded through the pipeline.
#[derive(Debug, Default)]
pub struct PipelineContext {
    /// Diagnostics collected by stages (warnings, info, timing).
    pub diagnostics: Vec<Diagnostic>,
    /// Arbitrary key-value metadata that stages can read/write.
    pub metadata: HashMap<String, MetadataValue>,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub stage: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub enum MetadataValue {
    Bool(bool),
    Usize(usize),
    String(String),
    TimePoint(TimePoint),
}

/// A single, self-contained transformation over a `Theory`.
///
/// Stages are applied left-to-right. Each stage receives the theory
/// produced by the previous stage and a shared `PipelineContext` for
/// diagnostics and inter-stage communication.
pub trait PipelineStage: std::fmt::Debug {
    /// Human-readable name used in diagnostics and tracing.
    fn name(&self) -> &'static str;

    /// Apply this stage, returning a (possibly transformed) theory.
    ///
    /// Returning `Err` aborts the pipeline. To report a non-fatal
    /// problem, push a `Diagnostic` onto `ctx` and return `Ok`.
    fn apply(&self, theory: Theory, ctx: &mut PipelineContext) -> Result<Theory>;
}
```

### Built-in stages

Each current phase becomes a standalone struct implementing `PipelineStage`.

```rust
/// Validates wildcard placement and range restriction.
#[derive(Debug, Clone)]
pub struct Validate {
    pub enforce_range_restricted: bool,
    pub reject_wildcard_in_head: bool,
}

impl Default for Validate {
    fn default() -> Self {
        Self {
            enforce_range_restricted: true,
            reject_wildcard_in_head: true,
        }
    }
}

impl PipelineStage for Validate {
    fn name(&self) -> &'static str { "validate" }

    fn apply(&self, theory: Theory, ctx: &mut PipelineContext) -> Result<Theory> {
        if self.reject_wildcard_in_head {
            validate_wildcards(&theory)?;
        }
        if self.enforce_range_restricted {
            validate_range_restriction(&theory)?;
        }
        ctx.diagnostics.push(Diagnostic {
            severity: Severity::Info,
            stage: self.name(),
            message: "validation passed".into(),
        });
        Ok(theory)
    }
}

/// Filters the theory to include only rules/facts active at a reference time.
#[derive(Debug, Clone)]
pub struct TemporalFilter {
    pub reference_time: TimePoint,
}

impl PipelineStage for TemporalFilter {
    fn name(&self) -> &'static str { "temporal_filter" }

    fn apply(&self, theory: Theory, ctx: &mut PipelineContext) -> Result<Theory> {
        let filtered = filter_temporal(&theory, self.reference_time);
        let removed = theory.rule_count() - filtered.rule_count();
        if removed > 0 {
            ctx.diagnostics.push(Diagnostic {
                severity: Severity::Info,
                stage: self.name(),
                message: format!("removed {removed} temporally inactive rules"),
            });
        }
        ctx.metadata.insert(
            "evaluated_at".into(),
            MetadataValue::TimePoint(self.reference_time),
        );
        Ok(filtered)
    }
}

/// Rewrites anonymous wildcards (`_`) to unique variables (`?_wN`).
#[derive(Debug, Clone, Copy)]
pub struct WildcardRewrite;

impl PipelineStage for WildcardRewrite {
    fn name(&self) -> &'static str { "wildcard_rewrite" }

    fn apply(&self, theory: Theory, _ctx: &mut PipelineContext) -> Result<Theory> {
        Ok(rewrite_wildcards(&theory))
    }
}

/// Bottom-up Datalog grounding of rules containing variables.
#[derive(Debug, Clone)]
pub struct Ground {
    pub max_iterations: usize,
    pub max_instances: usize,
}

impl Default for Ground {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            max_instances: 10_000,
        }
    }
}

impl PipelineStage for Ground {
    fn name(&self) -> &'static str { "ground" }

    fn apply(&self, theory: Theory, ctx: &mut PipelineContext) -> Result<Theory> {
        let had_vars = theory.rules().any(has_variables);
        if !had_vars {
            ctx.diagnostics.push(Diagnostic {
                severity: Severity::Info,
                stage: self.name(),
                message: "no variables found; grounding skipped".into(),
            });
            return Ok(theory);
        }

        let (grounded, limit_hit) = ground_theory_with_limit(
            &theory,
            self.max_iterations,
            self.max_instances,
        );

        let instances = grounded.rule_count();
        ctx.metadata.insert(
            "grounding_instances".into(),
            MetadataValue::Usize(instances),
        );
        ctx.metadata.insert(
            "grounding_limit_hit".into(),
            MetadataValue::Bool(limit_hit),
        );

        if limit_hit {
            ctx.diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                stage: self.name(),
                message: format!(
                    "grounding limit reached ({} instances, max {})",
                    instances, self.max_instances
                ),
            });
        }

        Ok(grounded)
    }
}
```

### Builder

```rust
/// A configured, ready-to-run pipeline.
#[derive(Debug)]
pub struct Pipeline {
    stages: Vec<Box<dyn PipelineStage>>,
}

impl Pipeline {
    /// Start building a pipeline with no stages.
    pub fn builder() -> PipelineBuilder {
        PipelineBuilder { stages: Vec::new() }
    }

    /// The default pipeline that reproduces current `prepare()` semantics.
    ///
    /// Equivalent to:
    /// ```rust
    /// Pipeline::builder()
    ///     .stage(Validate::default())
    ///     .stage(WildcardRewrite)
    ///     .stage(Ground::default())
    ///     .build()
    /// ```
    ///
    /// `TemporalFilter` is omitted by default because it requires a
    /// reference time that most callers do not provide.
    pub fn default_pipeline() -> Self {
        Self::builder()
            .stage(Validate::default())
            .stage(WildcardRewrite)
            .stage(Ground::default())
            .build()
    }

    /// Run all stages in order, returning the final theory and context.
    pub fn run(&self, theory: Theory) -> Result<(Theory, PipelineContext)> {
        let mut ctx = PipelineContext::default();
        let mut current = theory;

        for stage in &self.stages {
            current = stage.apply(current, &mut ctx)?;
        }

        Ok((current, ctx))
    }
}

#[derive(Debug)]
pub struct PipelineBuilder {
    stages: Vec<Box<dyn PipelineStage>>,
}

impl PipelineBuilder {
    /// Append a stage to the end of the pipeline.
    pub fn stage<S: PipelineStage + 'static>(mut self, s: S) -> Self {
        self.stages.push(Box::new(s));
        self
    }

    /// Insert a stage at a specific position (0-indexed).
    pub fn stage_at<S: PipelineStage + 'static>(mut self, index: usize, s: S) -> Self {
        self.stages.insert(index, Box::new(s));
        self
    }

    /// Consume the builder and produce an immutable `Pipeline`.
    pub fn build(self) -> Pipeline {
        Pipeline {
            stages: self.stages,
        }
    }
}
```

### Usage examples

**Default (backward-compatible):**

```rust
let pipeline = Pipeline::default_pipeline();
let (prepared_theory, ctx) = pipeline.run(theory)?;
let conclusions = reason_prepared(&prepared_theory)?;

// Inspect diagnostics
for d in &ctx.diagnostics {
    eprintln!("[{}] {}: {}", d.severity, d.stage, d.message);
}
```

**With temporal filtering:**

```rust
let pipeline = Pipeline::builder()
    .stage(TemporalFilter { reference_time: TimePoint::from_millis(now) })
    .stage(Validate::default())
    .stage(WildcardRewrite)
    .stage(Ground::default())
    .build();

let (theory, ctx) = pipeline.run(raw_theory)?;
```

**Custom stage (e.g., macro expansion):**

```rust
#[derive(Debug)]
struct ExpandMacros;

impl PipelineStage for ExpandMacros {
    fn name(&self) -> &'static str { "expand_macros" }

    fn apply(&self, mut theory: Theory, ctx: &mut PipelineContext) -> Result<Theory> {
        let expanded = my_macro_expander::expand(&theory);
        ctx.diagnostics.push(Diagnostic {
            severity: Severity::Info,
            stage: self.name(),
            message: format!("expanded {} macros", expanded.count),
        });
        Ok(expanded.theory)
    }
}

let pipeline = Pipeline::builder()
    .stage(ExpandMacros)
    .stage(Validate::default())
    .stage(WildcardRewrite)
    .stage(Ground::default())
    .build();
```

**Validation-only (e.g., editor lint):**

```rust
let pipeline = Pipeline::builder()
    .stage(Validate::default())
    .build();

let (_, ctx) = pipeline.run(theory)?;
// Only validation diagnostics, no grounding cost.
```

**Grounding with custom limits:**

```rust
let pipeline = Pipeline::builder()
    .stage(Validate::default())
    .stage(WildcardRewrite)
    .stage(Ground {
        max_iterations: 50,
        max_instances: 5_000,
    })
    .build();
```

### Stage ordering constraints

The following ordering invariants must hold for correct behavior. They are
documented here rather than enforced at compile time, because the trait-object
design makes static ordering proofs impractical without significant complexity.
A debug-mode runtime assertion in `Pipeline::run()` can validate the constraints.

| Constraint | Reason |
|------------|--------|
| `Validate` before `WildcardRewrite` | Validation rejects `_` in heads. Once wildcards are rewritten to `?_wN` variables, the check would no longer catch the original violation. |
| `WildcardRewrite` before `Ground` | Grounding matches variables (`?`-prefixed). Unrewritten `_` tokens are not variables and will not be instantiated, producing incorrect ground theories. |
| `TemporalFilter` before `Ground` | Filtering after grounding wastes work and may produce a different result if grounding derives facts from temporally-inactive rules. |
| `TemporalFilter` before `Validate` (recommended) | Filtering may remove rules that would otherwise fail validation, producing false positives. This ordering is recommended but not strictly required. |

The default pipeline produced by `Pipeline::default_pipeline()` satisfies all
constraints. Custom pipelines that violate them will still compile and run, but
may produce logically incorrect results.

### Backward compatibility

The existing public API (`prepare()` and `PrepareOptions`) is retained as a thin
wrapper:

```rust
pub fn prepare(theory: &Theory, opts: PrepareOptions) -> Result<PipelineResult> {
    let mut builder = Pipeline::builder();

    // 1. Temporal filter (optional)
    if let Some(t) = opts.reference_time {
        builder = builder.stage(TemporalFilter { reference_time: t });
    }

    // 2. Validation
    builder = builder.stage(Validate {
        enforce_range_restricted: opts.validation.enforce_range_restricted,
        reject_wildcard_in_head: opts.validation.reject_wildcard_in_head,
    });

    // 3. Wildcard rewrite
    builder = builder.stage(WildcardRewrite);

    // 4. Grounding
    if opts.grounding.enabled {
        builder = builder.stage(Ground {
            max_iterations: opts.grounding.max_iterations,
            max_instances: opts.grounding.max_instances,
        });
    }

    let pipeline = builder.build();
    let (mut theory, ctx) = pipeline.run(theory.clone())?;

    // Apply trust policy override
    if let Some(tp) = opts.trust_policy {
        *theory.trust_policy_mut() = tp;
    }

    // Reconstruct PipelineResult from context metadata
    let grounding_report = GroundingReport {
        performed: ctx.metadata.contains_key("grounding_instances"),
        had_variables: ctx.metadata.get("grounding_instances")
            .map(|v| matches!(v, MetadataValue::Usize(n) if *n > 0))
            .unwrap_or(false),
        instances: ctx.metadata.get("grounding_instances")
            .and_then(|v| if let MetadataValue::Usize(n) = v { Some(*n) } else { None })
            .unwrap_or(0),
        limit_hit: ctx.metadata.get("grounding_limit_hit")
            .and_then(|v| if let MetadataValue::Bool(b) = v { Some(*b) } else { None })
            .unwrap_or(false),
    };

    Ok(PipelineResult {
        theory,
        evaluated_at: opts.reference_time,
        grounding_report,
        weighted_conclusions: Vec::new(),
    })
}
```

All existing callers of `prepare()` continue to work without changes. The new
`Pipeline` API is purely additive.

## Consequences

### Positive

- **Extensibility.** Third-party and internal consumers can inject arbitrary
  stages (macro expansion, claim normalization, lint passes, instrumentation)
  without modifying core code.
- **Selective execution.** Editor integrations can run validation-only pipelines
  that skip grounding entirely, reducing latency from seconds to microseconds
  on large theories.
- **Structured diagnostics.** `PipelineContext` collects warnings and info
  messages from every stage, enabling rich editor feedback (e.g., "grounding
  limit approaching") that the current all-or-nothing `Result` cannot express.
- **Testability.** Each stage is independently unit-testable. Integration tests
  compose stages explicitly, making the test surface clearer.
- **Separation of concerns.** Configuration for grounding, validation, and
  temporal filtering lives in the stage structs rather than a monolithic options
  bag.

### Negative

- **Trait-object overhead.** Each `stage.apply()` call goes through dynamic
  dispatch. In practice the pipeline has 3-5 stages and each stage does
  substantial work (grounding alone is O(n^k)), so dispatch cost is negligible.
- **Theory cloning.** Stages consume and return `Theory` by value. Since
  `Theory` contains `Vec<Rule>` and metadata maps, this involves allocation.
  The current `prepare()` already clones the theory at least once (temporal
  filtering, wildcard rewrite), so the cost is comparable. Future optimization
  can introduce `Cow<Theory>` or arena-allocated theories.
- **Runtime ordering errors.** Stage ordering constraints are documented but
  not enforced at compile time. A misconfigured pipeline will silently produce
  wrong results. Mitigation: the default pipeline is correct, and a
  `debug_assert!`-based ordering check can catch common mistakes during
  development.
- **API surface growth.** The crate gains `PipelineStage`, `PipelineContext`,
  `Pipeline`, `PipelineBuilder`, `Diagnostic`, `Severity`, and `MetadataValue`
  types. This is offset by the eventual deprecation of `PrepareOptions` fields
  that duplicate stage configuration.

### Risks

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Custom stages mutate theory in ways that violate reasoning invariants (e.g., introduce unsafe rules after validation). | Medium | Document the contract: validation should run after any theory-mutating stage. Provide `Pipeline::default_pipeline_with(extras)` that inserts custom stages before validation. |
| Performance regression from additional cloning. | Low | Profile. `Theory::clone()` is already on the hot path. Consider `Cow` later. |
| Ordering constraint violations in user-built pipelines. | Medium | Add `Pipeline::validate_ordering()` method that checks known constraints and returns warnings. Call it in `debug_assert!`. |

## Alternatives Considered

### 1. Function-pointer chain (no trait)

Replace the trait with `Vec<fn(Theory, &mut PipelineContext) -> Result<Theory>>`.
Rejected because function pointers cannot carry configuration (e.g., grounding
limits) without closures, and closures are not `Debug`, making pipeline
introspection difficult.

### 2. Visitor pattern over an AST

Model the pipeline as a visitor that walks the theory's rule tree. Rejected
because stages like grounding fundamentally replace the entire theory rather
than modifying individual nodes, making the visitor pattern a poor fit.

### 3. Keep `prepare()` monolithic, add hooks

Add `before_grounding` / `after_validation` callback slots to `PrepareOptions`.
Rejected because this approach does not scale: each new extension point requires
a new option field, and the ordering between hooks and built-in phases remains
implicit.

### 4. Compile-time stage ordering via type-state builder

Encode ordering constraints in the type system (e.g.,
`PipelineBuilder<NeedsValidation>` -> `PipelineBuilder<NeedsGround>`). Rejected
because the combinatorial explosion of valid orderings (temporal filter is
optional, validation sub-checks are independent) would produce an impractical
number of type states.

## References

- Current `prepare()`: `crates/spindle-core/src/pipeline.rs:104-177`
- Grounding implementation: `crates/spindle-core/src/grounding.rs:315-457`
- Temporal filtering: `crates/spindle-core/src/pipeline.rs:180-228`
- Wildcard rewrite: `crates/spindle-core/src/pipeline.rs:284-347`
- Validation: `crates/spindle-core/src/pipeline.rs:230-282`
