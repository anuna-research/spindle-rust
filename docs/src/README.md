# Spindle-Rust

**Spindle-Rust** is a Rust implementation of the SPINdle defeasible logic reasoning engine.

This project is part of the SPINdle family:
- **[SPINdle](https://research.csiro.au/bpli/tools/spindle/)** ([source](https://sourceforge.net/projects/spindlereasoner/)) - The original Java implementation by NICTA (later Data61/CSIRO, now [CSIRO Technology](https://www.csiro.au/en/Newsletters/D61-NextGenConnect/2026-05))
- **[spindle-racket](https://codeberg.org/anuna/spindle-racket)** - A comprehensive Racket port of SPINdle Java v2.2.4, with trust-weighted reasoning
- **spindle-rust** - This Rust port, based on spindle-racket v1.7.0

## What is Defeasible Logic?

Defeasible logic is a [**non-monotonic reasoning**](https://www.youtube.com/watch?v=Ozipf13jRr4&t=1066) system that allows conclusions to be defeated by stronger evidence. Unlike classical logic where adding new information only adds new conclusions, defeasible logic can revise existing conclusions when conflicting evidence appears. Crucially, it is **tractable** — for propositional theories, inference runs in linear time relative to the size of the theory (Maher, 2001). With first-order variables, a grounding phase instantiates rules against known facts before reasoning begins; the grounding step can be exponential in rule body size (as with Datalog), but reasoning over the ground theory remains polynomial. This makes defeasible logic practical where other non-monotonic formalisms are intractable.

```spl
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
- **Reasoning engine:** traditional ambiguity-blocking DL(∂), with constructive negative tags
- **Temporal reasoning:** Allen interval algebra with 13 temporal relations
- **First-order variables:** Datalog-style grounding with `?x` syntax
- **Query operators:** status queries, what-if, why-not, abduction, and verified requirements
- **Aggregation:** grouped sum, count, minimum, and maximum over completed predicates
- **Extension functions:** host-registered pure functions and named aggregators
- **Predicate vocabulary:** declarations, metadata, and non-semantic shape diagnostics
- **Verification:** Lean models and Rust/Lean differential tests for supported fragments
- **Trust-aware reasoning:** source attribution and weighted conclusions
- **SPL format:** Lisp-based input with variables, temporal, and trust directives
- **WebAssembly support:** run in browsers and Node.js

See [Aggregation and Extension Functions](guides/aggregation.md),
[Query Operators](guides/queries.md), and [Verification](internals/verification.md)
for current behavior and supported boundaries.

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
let conclusions = theory.reason().expect("reasoning succeeds");
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
| `spindle-contract` | Shared JSON contracts |
| `spindle-wasm` | WebAssembly bindings |

## References

- [SPINdle Project](https://research.csiro.au/bpli/tools/spindle/) - Original Java implementation by NICTA (later Data61/CSIRO, now [CSIRO Technology](https://www.csiro.au/en/Newsletters/D61-NextGenConnect/2026-05))
- Nute, D. (1994). "Defeasible Logic" - Foundational paper on defeasible logic
- [spindle-racket](https://codeberg.org/anuna/spindle-racket) - Racket implementation this port is based on

## License

LGPL-3.0-or-later (same as original SPINdle)
