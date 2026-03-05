# Spindle-Rust

**Spindle-Rust** is a Rust implementation of the SPINdle defeasible logic reasoning engine.

This project is part of the SPINdle family:
- **[SPINdle](http://spindle.data61.csiro.au/)** ([source](https://sourceforge.net/projects/spindlereasoner/)) - The original Java implementation by NICTA (now Data61/CSIRO)
- **[spindle-racket](https://codeberg.org/anuna/spindle-racket)** - A comprehensive Racket port of SPINdle Java v2.2.4, with trust-weighted reasoning
- **spindle-rust** - This Rust port, based on spindle-racket v1.7.0

## What is Defeasible Logic?

Defeasible logic is a **non-monotonic reasoning** system that allows conclusions to be defeated by stronger evidence. Unlike classical logic where adding new information only adds new conclusions, defeasible logic can revise existing conclusions when conflicting evidence appears.

```lisp
; The classic "Tweety" example
(given bird tweety)              ; Tweety is a bird
(given penguin tweety)           ; Tweety is a penguin

(normally birds-fly              ; Birds typically fly
  (bird ?x) (flies ?x))

(normally penguins-dont-fly      ; Penguins don't fly
  (penguin ?x) (not (flies ?x)))

(prefer penguins-dont-fly birds-fly)  ; Specificity: more specific rule prevails
```

**Result:**

```
Conclusions:

  +D penguin(tweety)
  +D bird(tweety)
  +d penguin(tweety)
  +d bird(tweety)
  +d ~flies(tweety)
  -D flies(tweety)
  -d flies(tweety)
  -D ~flies(tweety)
```

Tweety is defeasibly proven not to fly (`+d ~flies(tweety)`), because `penguins-dont-fly` defeats `birds-fly`.

## Features

- **Four rule types:** facts, strict rules, defeasible rules, and defeaters
- **Reasoning engine:** standard DL(d) forward chaining
- **Temporal reasoning:** Allen interval algebra with 13 temporal relations
- **First-order variables:** Datalog-style grounding with `?x` syntax
- **Query operators:** what-if, why-not, and abduction
- **Trust-aware reasoning:** source attribution and weighted conclusions
- **SPL format:** Lisp-based input with variables, temporal, and trust directives
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
| `spindle-parser` | SPL format parser |
| `spindle-cli` | Command-line interface |
| `spindle-wasm` | WebAssembly bindings |

## References

- [SPINdle Project](http://spindle.data61.csiro.au/) - Original Java implementation by NICTA/Data61
- Nute, D. (1994). "Defeasible Logic" - Foundational paper on defeasible logic
- [spindle-racket](https://codeberg.org/anuna/spindle-racket) - Racket implementation this port is based on

## License

LGPL-3.0-or-later (same as original SPINdle)
