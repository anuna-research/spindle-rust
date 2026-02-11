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

fn main() {
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
    let conclusions = theory.reason();

    for c in &conclusions {
        if c.conclusion_type.is_positive() {
            println!("{}", c);
        }
    }
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

### Parsing DFL

```rust
use spindle_parser::parse_dfl;

let dfl = r#"
    f1: >> bird
    f2: >> penguin
    r1: bird => flies
    r2: penguin => ~flies
    r2 > r1
"#;

let theory = parse_dfl(dfl)?;
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

let conclusions = reason(&theory);
```

### Convenience Method

```rust
// Uses standard algorithm
let conclusions = theory.reason();
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

## Query Operators

### What-If

```rust
use spindle_core::query::{what_if, HypotheticalClaim};
use spindle_core::literal::Literal;

let hypotheticals = vec![
    HypotheticalClaim::new(Literal::simple("wounded"))
];
let goal = Literal::negated("flies");

let result = what_if(&theory, hypotheticals, &goal);
if result.is_provable() {
    println!("Would be provable with those facts");
}
```

### Why-Not

```rust
use spindle_core::query::why_not;
use spindle_core::literal::Literal;

let literal = Literal::simple("flies");
let explanation = why_not(&theory, &literal);
for blocker in &explanation.blocked_by {
    println!("Blocked by: {}", blocker.explanation);
}
```

### Abduction

```rust
use spindle_core::query::abduce;
use spindle_core::literal::Literal;

let goal = Literal::simple("goal");
let result = abduce(&theory, &goal, 3);
for solution in &result.solutions {
    println!("Solution: {:?}", solution.facts);
}
```

## Error Handling

```rust
use spindle_core::error::{SpindleError, Result};
use spindle_parser::ParseError;

fn load_theory(path: &str) -> Result<Theory> {
    let content = std::fs::read_to_string(path)?;

    if path.ends_with(".dfl") {
        parse_dfl(&content).map_err(|e| SpindleError::Parse(e.to_string()))
    } else if path.ends_with(".spl") {
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
use spindle_parser::parse_dfl;
use std::fs;

fn main() -> Result<()> {
    // Load theory
    let content = fs::read_to_string("rules.dfl")?;
    let mut theory = parse_dfl(&content)?;

    // Add runtime facts
    theory.add_fact("current_user_admin");

    // Reason
    let conclusions = theory.reason();

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
