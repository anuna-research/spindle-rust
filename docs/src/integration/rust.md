# Rust Library Guide

This guide covers using Spindle as a Rust library.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
spindle-core = { git = "https://codeberg.org/anuna/spindle-rust", package = "spindle-core" }
spindle-parser = { git = "https://codeberg.org/anuna/spindle-rust", package = "spindle-parser" }
```

## Basic Usage

```rust
use spindle_core::prelude::*;

fn main() -> Result<()> {
    let mut theory = Theory::new();

    // Add facts
    theory.add_fact("bird");
    theory.add_fact("penguin");

    // Add defeasible rules
    let r1 = theory.add_defeasible_rule(&["bird"], "flies");
    let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");

    // Set superiority
    theory.add_superiority(&r2, &r1);

    // Reason
    let conclusions = theory.reason()?;

    for c in &conclusions {
        if c.conclusion_type.is_positive() {
            println!("{}", c);
        }
    }

    Ok(())
}
```

## Creating Theories

### Programmatic Construction

```rust
use spindle_core::prelude::*;

let mut theory = Theory::new();

// Facts
theory.add_fact("bird");
theory.add_fact("~guilty");  // Negated fact

// Strict rules
theory.add_strict_rule(&["penguin"], "bird");

// Defeasible rules
let r1 = theory.add_defeasible_rule(&["bird"], "flies");
let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");

// Defeaters
theory.add_defeater(&["broken_wing"], "flies");

// Superiority
theory.add_superiority(&r2, &r1);
```

### Parsing SPL

```rust
use spindle_parser::parse_spl;

let spl = r#"
    (given bird)
    (given penguin)
    (normally r1 bird flies)
    (normally r2 penguin (not flies))
    (prefer r2 r1)
"#;

let theory = parse_spl(spl)?;
```

## Reasoning

### Reasoning API

```rust
use spindle_core::reason::reason;

let conclusions = reason(&theory)?;
```

### Convenience Method

```rust
// Uses standard algorithm
let conclusions = theory.reason()?;
```

## Working with Conclusions

### Conclusion Types

```rust
use spindle_core::conclusion::ConclusionType;

for c in &conclusions {
    match c.conclusion_type {
        ConclusionType::DefinitelyProvable => {
            println!("+D {}", c.literal);
        }
        ConclusionType::DefinitelyNotProvable => {
            println!("-D {}", c.literal);
        }
        ConclusionType::DefeasiblyProvable => {
            println!("+d {}", c.literal);
        }
        ConclusionType::DefeasiblyNotProvable => {
            println!("-d {}", c.literal);
        }
    }
}
```

### Filtering Conclusions

```rust
// Positive conclusions only
let positive: Vec<_> = conclusions
    .iter()
    .filter(|c| c.conclusion_type.is_positive())
    .collect();

// Defeasibly provable only
let defeasible: Vec<_> = conclusions
    .iter()
    .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
    .collect();
```

### Checking Specific Literals

```rust
fn is_provable(conclusions: &[Conclusion], name: &str) -> bool {
    conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == name
            && !c.literal.negation
    })
}

if is_provable(&conclusions, "flies") {
    println!("It flies!");
}
```

## Terms

The `Term` enum represents typed values that appear as arguments in literals
and as results of arithmetic expressions. Four variants cover the value space:

```rust
use spindle_core::term::{Term, FiniteFloat, NumericValue};
use spindle_core::intern::{SymbolId, intern};
use rust_decimal::Decimal;

// Symbol — an interned string identifier
let sym = Term::Symbol(intern("alice"));

// Integer — a 64-bit signed integer
let int = Term::Integer(42);

// Decimal — an arbitrary-precision decimal (exact representation)
let dec = Term::Decimal(Decimal::new(314, 2)); // 3.14

// Float — a canonicalized finite IEEE 754 f64
let flt = Term::Float(FiniteFloat::new(2.718).unwrap());
```

### Convenience Conversions

Terms implement `From` for common types, so you can use `.into()`:

```rust
let t: Term = 42i64.into();               // Term::Integer(42)
let t: Term = Decimal::ONE.into();         // Term::Decimal(1)
let t: Term = intern("bob").into();        // Term::Symbol(...)
let t: Term = FiniteFloat::new(1.0)
    .unwrap().into();                      // Term::Float(1.0)
```

### Numeric Checks and Cross-Type Equality

`Term::is_numeric()` returns `true` for Integer, Decimal, and Float variants.
`Term::numeric_eq()` supports cross-type comparison with widening promotion
(Integer -> Decimal -> Float):

```rust
let a = Term::Integer(2);
let b = Term::Decimal(Decimal::new(20, 1)); // 2.0
assert!(a.numeric_eq(&b)); // true — same mathematical value

let c = Term::Float(FiniteFloat::new(2.0).unwrap());
assert!(a.numeric_eq(&c)); // true
```

Symbols are never numerically equal to anything (including other symbols).

### NumericValue

`NumericValue` is a guaranteed-numeric counterpart to `Term` (no `Symbol`
variant). Convert with `Term::to_numeric_value()`:

```rust
let term = Term::Integer(10);
if let Some(nv) = term.to_numeric_value() {
    println!("Numeric: {}", nv); // "10"
}

// Convert back (Float variant rejects NaN/Inf)
let back: Term = NumericValue::Integer(10).try_into().unwrap();
```

### FiniteFloat

`FiniteFloat` wraps `f64` with two invariants: it rejects NaN and infinity,
and normalizes `-0.0` to `+0.0`. This makes it safe to use as a hash-map key
with stable `Eq` and `Hash` implementations.

```rust
let f = FiniteFloat::new(3.14).unwrap();
assert_eq!(f.value(), 3.14);

// NaN and infinity are rejected
assert!(FiniteFloat::new(f64::NAN).is_none());
assert!(FiniteFloat::new(f64::INFINITY).is_none());

// Negative zero normalizes to positive zero
let a = FiniteFloat::new(0.0).unwrap();
let b = FiniteFloat::new(-0.0).unwrap();
assert_eq!(a, b);
```

## Literals

### Creating Literals

```rust
use spindle_core::literal::Literal;

let simple = Literal::simple("bird");
let negated = Literal::negated("flies");
let with_args = Literal::new(
    "parent",
    false,  // not negated
    Default::default(),  // no mode
    Default::default(),  // no temporal
    vec!["alice".to_string(), "bob".to_string()],
);
```

### Literal Properties

```rust
let lit = Literal::negated("flies");

println!("Name: {}", lit.name());           // "flies"
println!("Negated: {}", lit.is_negated());  // true
println!("Canonical: {}", lit.canonical_name()); // "~flies"

let complement = lit.complement();  // Literal::simple("flies")
```

## Rules

### Creating Rules Directly

```rust
use spindle_core::rule::{Rule, RuleType};

let rule = Rule::new(
    "r1",
    RuleType::Defeasible,
    vec![Literal::simple("bird")],
    vec![Literal::simple("flies")],
);

theory.add_rule(rule);
```

### Inspecting Rules

```rust
for rule in theory.rules() {
    println!("Label: {}", rule.label);
    println!("Type: {:?}", rule.rule_type);
    println!("Body: {:?}", rule.body);
    println!("Head: {:?}", rule.head);
}
```

## Pipeline

The preparation pipeline transforms a raw `Theory` into a form ready for
reasoning. It is built from composable stages assembled via `PipelineBuilder`.

### Default Pipeline

The simplest way to prepare a theory uses `prepare()` with default options:

```rust
use spindle_core::pipeline::{prepare, PrepareOptions};

let result = prepare(&theory, PrepareOptions::default())?;
let prepared_theory = result.theory;
```

### PipelineBuilder

For fine-grained control, build a custom pipeline from individual stages:

```rust
use spindle_core::pipeline::{Pipeline, Validate, WildcardRewrite, Ground};

let pipeline = Pipeline::builder()
    .stage(Validate::default())
    .stage(WildcardRewrite)
    .stage(Ground::default())
    .build();

let (prepared_theory, ctx) = pipeline.run(theory)?;
```

Stages run left-to-right. Each stage receives the theory produced by the
previous stage and a shared `PipelineContext`.

You can insert a stage at a specific position with `stage_at()`:

```rust
use spindle_core::pipeline::{Pipeline, Validate, WildcardRewrite, Ground, TemporalFilter};
use spindle_core::temporal::TimePoint;

let pipeline = Pipeline::builder()
    .stage(Validate::default())
    .stage(WildcardRewrite)
    .stage(Ground::default())
    .stage_at(0, TemporalFilter { reference_time: TimePoint::Moment(1700000000000) })
    .build();
```

### Built-in Stages

| Stage | Purpose |
|-------|---------|
| `Validate` | Enforces range restriction and rejects wildcards in rule heads. Both checks are independently configurable. |
| `WildcardRewrite` | Rewrites anonymous wildcards (`_`) to unique fresh variables (`?_wN`). |
| `Ground` | Bottom-up Datalog grounding. Configurable `max_iterations` and `max_instances` limits. |
| `TemporalFilter` | Removes rules/facts not active at a given reference `TimePoint`. |
| `TemporalVarValidation` | Rejects theories with unresolved temporal variables after grounding. |

### Configuring Stages

```rust
use spindle_core::pipeline::{Validate, Ground};

// Disable range restriction check
let validate = Validate {
    enforce_range_restricted: false,
    reject_wildcard_in_head: true,
};

// Increase grounding limits
let ground = Ground {
    max_iterations: 200,
    max_instances: 50000,
};
```

### Implementing Custom Stages

Any type implementing the `PipelineStage` trait can be added to a pipeline:

```rust
use spindle_core::pipeline::{PipelineStage, PipelineContext, Severity, Diagnostic};
use spindle_core::theory::Theory;
use spindle_core::error::Result;

#[derive(Debug)]
struct LogRuleCount;

impl PipelineStage for LogRuleCount {
    fn name(&self) -> &'static str { "LogRuleCount" }

    fn apply(&self, theory: Theory, ctx: &mut PipelineContext) -> Result<Theory> {
        ctx.diagnostics.push(Diagnostic {
            severity: Severity::Info,
            stage: self.name(),
            message: format!("Theory has {} rules", theory.rules().len()),
        });
        Ok(theory)
    }
}
```

### PipelineContext and Diagnostics

A `PipelineContext` is threaded through all stages, collecting diagnostics and
inter-stage metadata.

```rust
use spindle_core::pipeline::{Pipeline, Validate, WildcardRewrite, Ground, Severity, MetadataVal};

let pipeline = Pipeline::builder()
    .stage(Validate::default())
    .stage(WildcardRewrite)
    .stage(Ground::default())
    .build();

let (prepared, ctx) = pipeline.run(theory)?;

// Inspect diagnostics
for diag in &ctx.diagnostics {
    match diag.severity {
        Severity::Error   => eprintln!("[{}] ERROR: {}", diag.stage, diag.message),
        Severity::Warning => eprintln!("[{}] WARN:  {}", diag.stage, diag.message),
        Severity::Info    => println!("[{}] INFO:  {}", diag.stage, diag.message),
    }
}

// Read metadata set by stages (e.g., grounding statistics)
if let Some(MetadataVal::Usize(n)) = ctx.metadata.get("grounding_instances") {
    println!("Grounding produced {} instances", n);
}
if let Some(MetadataVal::Bool(true)) = ctx.metadata.get("grounding_limit_hit") {
    eprintln!("Warning: grounding limit was reached");
}
```

### PrepareOptions

The `prepare()` function builds a full pipeline from `PrepareOptions`,
including optional temporal filtering and grounding configuration:

```rust
use spindle_core::pipeline::{prepare, PrepareOptions, GroundingOptions, ValidationOptions};
use spindle_core::temporal::TimePoint;

let opts = PrepareOptions {
    reference_time: Some(TimePoint::Moment(1700000000000)),
    grounding: GroundingOptions {
        enabled: true,
        max_iterations: 100,
        max_instances: 10000,
    },
    validation: ValidationOptions {
        enforce_range_restricted: true,
        reject_wildcard_in_head: true,
    },
    trust_policy: None,
};

let result = prepare(&theory, opts)?;

// Access the prepared theory and grounding statistics
let theory = result.theory;
println!("Grounded: {}", result.grounding_report.performed);
println!("Instances: {}", result.grounding_report.instances);
if result.grounding_report.limit_hit {
    eprintln!("Grounding stopped early due to limits");
}
```

## Query Operators

### What-If

```rust
use spindle_core::query::{what_if, HypotheticalClaim};
use spindle_core::literal::Literal;

let hypotheticals = vec![
    HypotheticalClaim::new(Literal::simple("wounded"))
];
let goal = Literal::negated("flies");

let result = what_if(&theory, hypotheticals, &goal)?;
if result.is_provable() {
    println!("Would be provable with those facts");
}
for lit in result.newly_provable() {
    println!("Newly provable: {}", lit);
}
```

### Why-Not

```rust
use spindle_core::query::why_not;
use spindle_core::literal::Literal;

let literal = Literal::simple("flies");
let explanation = why_not(&theory, &literal)?;
for blocker in &explanation.blocked_by {
    println!("Blocked by: {}", blocker.explanation);
}
```

### Abduction

```rust
use spindle_core::query::abduce;
use spindle_core::literal::Literal;

let goal = Literal::simple("goal");
let result = abduce(&theory, &goal, 3)?;
for solution in &result.solutions {
    println!("Solution: {:?}", solution.facts);
}
```

## Error Handling

```rust
use spindle_core::error::{SpindleError, Result};
use spindle_parser::parse_spl;

fn load_theory(path: &str) -> Result<Theory> {
    let content = std::fs::read_to_string(path)?;

    if path.ends_with(".spl") {
        parse_spl(&content).map_err(|e| SpindleError::Parse(e.to_string()))
    } else {
        Err(SpindleError::Parse("Unknown format".to_string()))
    }
}
```

## Advanced: Indexed Theory

For multiple queries, build an indexed theory once:

```rust
use spindle_core::index::IndexedTheory;

let indexed = IndexedTheory::build(theory.clone());

// O(1) lookups
for rule in indexed.rules_with_head(&literal) {
    // Process rules that conclude this literal
}

for rule in indexed.rules_with_body(&literal) {
    // Process rules that have this literal in body
}
```

## Thread Safety

`Theory` and `Conclusion` are `Send + Sync`. You can:

```rust
use std::sync::Arc;
use rayon::prelude::*;

let theory = Arc::new(theory);

let results: Vec<_> = queries
    .par_iter()
    .map(|query| {
        let t = theory.clone();
        process_query(&t, query)
    })
    .collect();
```

## Example: Complete Application

```rust
use spindle_core::prelude::*;
use spindle_parser::parse_spl;
use std::fs;

fn main() -> Result<()> {
    // Load theory
    let content = fs::read_to_string("rules.spl")?;
    let mut theory = parse_spl(&content)?;

    // Add runtime facts
    theory.add_fact("current_user_admin");

    // Reason
    let conclusions = theory.reason()?;

    // Check permissions
    let can_delete = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == "can_delete"
    });

    if can_delete {
        println!("User can delete");
    } else {
        println!("Permission denied");
    }

    Ok(())
}
```

## Predicate Vocabulary

SPEC-024 adds a structural predicate model on top of the reasoning AST. It is
entirely additive and **non-semantic**: constructing signatures, vocabularies,
or shapes never changes conclusions. The types live in
`spindle_core::vocabulary` and are re-exported from the prelude.

### Predicate Symbols

A `PredicateSymbol` is the pair `(functor, arity)` — the structural identity of
a predicate, excluding arguments, polarity, mode, and temporal bounds. Any head
literal or logical body literal projects to one:

```rust
use spindle_core::prelude::*;

let lit = Literal::new("assign-to", false, Mode::empty(), Default::default(),
    vec!["t1".into(), "alice".into()]);
let sym = lit.predicate_symbol().unwrap();   // via HasPredicateSymbol
assert_eq!(sym.arity(), 2);
assert_eq!(sym.indicator().to_string(), "assign-to/2");
```

`p(a)` and `~p(b)` share `p/1`, but this identity is deliberately *not* a
proof-state key — `FamilyId`, `LitId`, and `ExactLitId` remain authoritative for
reasoning.

### Declaring Predicates Programmatically

```rust
use spindle_core::prelude::*;

let mut theory = Theory::new();
let sig = PredicateSignature::try_new(
    PredicateSymbol::try_new("assign-to".into(), 2).unwrap(),
    vec![
        ArgumentDecl::new("task", PrimitiveSort::Symbol),
        ArgumentDecl::new("agent", PrimitiveSort::Symbol),
    ],
).unwrap();
theory.add_predicate_declaration(
    PredicateDeclaration::new(sig, DeclarationOrigin::Programmatic));

// Predicate-targeted metadata (distinct from rule-label metadata):
theory.add_meta_target(
    MetaTarget::Predicate(PredicateSymbol::try_new("assign-to".into(), 2).unwrap()),
    "description",
    MetaValue::String("Assign a task to an agent.".into()),
);
```

`PredicateSignature::try_new` enforces that the argument count equals the arity
and that names are non-empty and unique.

### Deriving the Theory Signature and Vocabulary

`TheorySignature::derive` returns every predicate symbol the theory uses or
declares, plus each symbol's declaration state (`Declared` or `Conflict`).
`Vocabulary::derive` builds the fuller catalogue and its diagnostics:

```rust
use spindle_core::prelude::*;

let report = Vocabulary::derive(&theory);
for entry in &report.vocabulary.entries {
    println!("{}", entry.symbol.indicator());
    if let Some(desc) = &entry.description {
        println!("  {desc}");
    }
    // entry.profile — observed argument kinds per position
    // entry.origins — rule occurrences, sorted by (label, head-before-body, index)
}

// Deterministic summary counts (OBS-001):
println!("{} symbols, {} occurrences, {} conflicts",
    report.summary.distinct_symbols,
    report.summary.observed_occurrences,
    report.summary.declaration_conflicts);
```

Entries are ordered by `(functor, arity)` and the output is independent of
`HashMap` iteration order.

### Literal Phases

`GroundLiteral` and `LiteralPattern` are checked wrappers that make illegal
phase transitions unrepresentable. `Literal::classify` places every literal in
exactly one:

```rust
use spindle_core::prelude::*;

match Literal::simple("bird").classify() {
    ClassifiedLiteral::Ground(g) => { /* g.as_literal() has no variables */ }
    ClassifiedLiteral::Pattern(p) => { /* p.ground(&bindings) -> GroundLiteral */ }
}
```

### Shape Validation

A `Shape` compiled from a signature validates argument sorts at a boundary
(e.g. before an effectful action). It is never consulted by the reasoner:

```rust
use spindle_core::prelude::*;

let shape = Shape::from(&sig);
let report = shape.validate(&lit).unwrap();
// report.diagnostics: SortMismatch / PredicateMismatch / Deferred (for variables)
```

### Parsing Predicate Indicators

The `spindle-parser` crate exposes a fully-consuming recognizer for the
`functor/arity` notation (slash-bearing functors must be quoted):

```rust
use spindle_parser::parse_predicate_indicator;

let sym = parse_predicate_indicator("assign-to/2").unwrap();
assert!(parse_predicate_indicator("rate/limit/2").is_err());        // ambiguous
let quoted = parse_predicate_indicator("\"rate/limit\"/2").unwrap(); // ok
```
