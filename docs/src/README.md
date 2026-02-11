# Spindle-Rust

**Spindle-Rust** is a Rust implementation of the SPINdle defeasible logic reasoning engine.

This project is part of the SPINdle family:
- **[SPINdle](http://spindle.data61.csiro.au/)** - The original Java implementation by NICTA (now Data61/CSIRO)
- **[spindle-racket](https://codeberg.org/anuna/spindle-racket)** - A comprehensive Racket port with trust-weighted reasoning
- **spindle-rust** - This Rust port, based on spindle-racket v1.7.0

## What is Defeasible Logic?

Defeasible logic is a **non-monotonic reasoning** system that allows conclusions to be defeated by stronger evidence. Unlike classical logic where adding new information only adds new conclusions, defeasible logic can revise existing conclusions when conflicting evidence appears.

```dfl
# The classic "Tweety" example
f1: >> bird
f2: >> penguin

r1: bird => flies        # Birds typically fly
r2: penguin => -flies    # Penguins don't fly

r2 > r1                  # Penguin rule beats bird rule
```

**Result:** `-flies` is provable (Tweety doesn't fly).

## Features

- **Four rule types:** facts, strict rules, defeasible rules, and defeaters
- **Reasoning engine:** standard DL(d) forward chaining
- **Temporal reasoning:** Allen interval algebra with 13 temporal relations
- **First-order variables:** Datalog-style grounding with `?x` syntax
- **Query operators:** what-if, why-not, and abduction
- **Trust-aware reasoning:** source attribution and weighted conclusions
- **Two input formats:** DFL (textual) and SPL (Lisp-based)
- **WebAssembly support:** run in browsers and Node.js

## Quick Example

```rust
use spindle_core::prelude::*;

let mut theory = Theory::new();

// Add facts
theory.add_fact("bird");
theory.add_fact("penguin");

// Add defeasible rules
let r1 = theory.add_defeasible_rule(&["bird"], "flies");
let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");

// Penguin rule beats bird rule
theory.add_superiority(&r2, &r1);

// Reason and get conclusions
let conclusions = theory.reason();
```

## Installation

### CLI

```bash
cargo install --path crates/spindle-cli
```

### Library

```toml
[dependencies]
spindle-core = { path = "crates/spindle-core" }
spindle-parser = { path = "crates/spindle-parser" }
```

## Crate Structure

| Crate | Description |
|-------|-------------|
| `spindle-core` | Core reasoning engine |
| `spindle-parser` | DFL and SPL format parsers |
| `spindle-cli` | Command-line interface |
| `spindle-wasm` | WebAssembly bindings |

## References

- [SPINdle Project](http://spindle.data61.csiro.au/) - Original Java implementation by NICTA/Data61
- Nute, D. (1994). "Defeasible Logic" - Foundational paper on defeasible logic
- [spindle-racket](https://codeberg.org/anuna/spindle-racket) - Racket implementation this port is based on

## License

LGPL-3.0-or-later (same as original SPINdle)
