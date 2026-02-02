# Spindle-Rust

A Rust implementation of the [SPINdle](http://spindle.data61.csiro.au/) defeasible logic reasoning engine.

This project is part of the SPINdle family:
- **[SPINdle](http://spindle.data61.csiro.au/)** - The original Java implementation (v2.2.4) by NICTA (now Data61/CSIRO)
- **[spindle-racket](https://codeberg.org/anuna/spindle-racket)** - A Racket port with trust-weighted reasoning and `#lang spindle`
- **spindle-rust** - This Rust port, based on spindle-racket v1.7.0

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

- **First-Order Variables**: Datalog-style grounding with `?x` variable syntax

- **Two Input Formats**:
  - DFL (Defeasible Logic Format) - Textual syntax
  - SPL (Spindle Lisp) - LISP-based DSL

- **Explanations**: Proof trees with natural language and JSON output

- **Trust-Aware Reasoning**: Source attribution and trust-weighted conclusions

- **Query Operators**:
  - What-if: Hypothetical reasoning
  - Why-not: Explanation of failures
  - Abduction: Finding hypotheses to prove goals

- **WebAssembly Support**: Run in browsers and Node.js via wasm-bindgen

## Installation

```bash
cargo install --path crates/spindle-cli
```

## Usage

```bash
# Reason about a theory (DFL format)
spindle examples/penguin.dfl

# Reason about a theory (SPL format)
spindle examples/penguin.spl

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

## SPL Format

```lisp
; Facts
(given bird)
(given penguin)

; Defeasible rules
(normally r1 bird flies)
(normally r2 penguin (not flies))

; Superiority
(prefer r2 r1)

; Predicates with variables
(given (parent alice bob))
(normally r3 (parent ?x ?y) (ancestor ?x ?y))
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

## WebAssembly Usage

Build the WASM package:

```bash
cd crates/spindle-wasm
wasm-pack build --target web --release
```

Use in JavaScript/TypeScript:

```typescript
import init, { Spindle } from 'spindle-wasm';

await init();

const spindle = new Spindle();

// Add theory programmatically
spindle.addFact("bird");
spindle.addFact("penguin");
spindle.addDefeasibleRule(["bird"], "flies");
spindle.addDefeasibleRule(["penguin"], "~flies");
spindle.addSuperiority("r2", "r1");

// Or parse DFL
spindle.parseDfl(`
  f1: >> bird
  f2: >> penguin
  r1: bird => flies
  r2: penguin => ~flies
  r2 > r1
`);

// Reason
const conclusions = spindle.reason();
// => [{conclusion_type: "+D", literal: "bird", positive: true}, ...]

// Query
const result = spindle.query("~flies");
// => {status: "provable", literal: "~flies", conclusion_type: "+d"}

// What-if hypothetical reasoning
const whatIf = spindle.whatIf(["wounded"], "~flies");
// => {provable: true, new_conclusions: [...]}

// Why-not failure explanation
const whyNot = spindle.whyNot("flies");
// => {literal: "flies", would_derive: "r1", blockers: [...]}

// Abduction
const abduce = spindle.abduce("flies", 3);
// => {goal: "flies", solutions: [[...], [...]]}
```

## Crate Structure

- `spindle-core` - Core reasoning engine
  - `reason` - Standard DL(d) forward chaining
  - `scalable` - Scalable DL(d||) three-phase algorithm
  - `temporal` - Allen interval algebra
  - `grounding` - Datalog-style variable grounding
  - `explanation` - Proof trees and explanations
  - `trust` - Trust-weighted reasoning
  - `query` - What-if, why-not, abduction operators
- `spindle-parser` - DFL and SPL format parsers
- `spindle-cli` - Command-line interface
- `spindle-wasm` - WebAssembly bindings for JavaScript/TypeScript

## Testing

128 tests covering:
- Core reasoning (facts, rules, conflicts, superiority)
- Semantic equivalence (standard vs scalable modes)
- Edge cases (cycles, empty theories, defeaters)
- Stress tests (long chains, wide theories)
- Query operators (what-if, why-not, abduction)

```bash
cargo test
```

## Documentation

Full documentation is available at [docs/](docs/) or build locally:

```bash
cd docs && mdbook serve
```

## References

- [SPINdle Project](http://spindle.data61.csiro.au/) - Original Java implementation by NICTA/Data61
- Nute, D. (1994). "Defeasible Logic" - Foundational paper
- [spindle-racket](https://codeberg.org/anuna/spindle-racket) - Racket implementation

## License

LGPL-3.0-or-later (same as original SPINdle)
