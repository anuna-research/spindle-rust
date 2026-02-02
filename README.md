# Spindle-Rust

A Rust implementation of the SPINdle defeasible logic reasoning engine, ported from [spindle-racket](https://github.com/anthropics/spindle-racket) v1.7.0.

## Features

- **Defeasible Logic Reasoning**: Implements non-monotonic reasoning with four rule types:
  - Facts (`>>`) - Unconditional truths
  - Strict rules (`->`) - Must hold if antecedent is true
  - Defeasible rules (`=>`) - Normally hold unless defeated
  - Defeaters (`~>`) - Block conclusions without proving anything

- **Two Reasoning Modes**:
  - Standard DL(d) - Traditional forward chaining
  - Scalable DL(d||) - Three-phase closure algorithm for large theories

- **Temporal Reasoning**: Allen interval algebra with 13 temporal relations

- **Superiority Relations**: Conflict resolution via rule preferences

## Installation

```bash
cargo install --path crates/spindle-cli
```

## Usage

```bash
# Reason about a theory
spindle examples/penguin.dfl

# Use scalable mode
spindle --scalable examples/penguin.dfl

# Show only positive conclusions
spindle --positive examples/penguin.dfl

# Validate a theory file
spindle validate examples/penguin.dfl

# Show statistics
spindle stats examples/penguin.dfl
```

## DFL Format

```dfl
# Facts
f1: >> bird
f2: >> penguin

# Defeasible rules
r1: bird => flies
r2: penguin => -flies

# Superiority (r2 beats r1)
r2 > r1
```

## Library Usage

```rust
use spindle_core::prelude::*;

let mut theory = Theory::new();

// Add facts
theory.add_fact("bird");
theory.add_fact("penguin");

// Add rules
let r1 = theory.add_defeasible_rule(&["bird"], "flies");
let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");

// Superiority: penguins override birds
theory.add_superiority(&r2, &r1);

// Reason
let conclusions = theory.reason();
```

## Crate Structure

- `spindle-core` - Core reasoning engine and data structures
- `spindle-parser` - DFL format parser
- `spindle-cli` - Command-line interface

## License

LGPL-3.0-or-later (same as original SPINdle)
