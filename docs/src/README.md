# Spindle-Rust

**Spindle-Rust** is a Rust implementation of the SPINdle defeasible logic reasoning engine.

This project is part of the SPINdle family:
- **[SPINdle](https://research.csiro.au/bpli/tools/spindle/)** ([source](https://sourceforge.net/projects/spindlereasoner/)) - The original Java implementation by NICTA (later Data61/CSIRO, now [CSIRO Technology](https://www.csiro.au/en/Newsletters/D61-NextGenConnect/2026-05))
- **[spindle-racket](https://codeberg.org/anuna/spindle-racket)** - A comprehensive Racket port of SPINdle Java v2.2.4, with trust-weighted reasoning
- **spindle-rust** - This Rust port, based on spindle-racket v1.7.0

## What is Defeasible Logic?

Defeasible logic is a [**non-monotonic reasoning**](https://www.youtube.com/watch?v=Ozipf13jRr4&t=1066) system. Stronger evidence can defeat its conclusions.
Classical logic only adds conclusions when new information arrives. Defeasible logic can revise conclusions when conflicting evidence appears.

Tractable inference makes defeasible logic practical where other non-monotonic formalisms are intractable.
[Algorithms](guides/algorithms.md#theoretical-complexity-and-grounding) explains theoretical complexity and the separate cost of grounding first-order variables.

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

- **[Rules](concepts/rules.md) and [reasoning](guides/algorithms.md):** facts, strict rules, defeasible rules, and defeaters. The engine implements traditional ambiguity-blocking DL(∂) with constructive negative tags.
- **[Variables](guides/grounding.md) and [time](guides/temporal.md):** Datalog-style grounding with `?x` syntax; Allen interval algebra with 13 temporal relations.
- **[Queries](guides/queries.md):** status queries, what-if, why-not, abduction, and verified requirements.
- **[Aggregation and extensions](guides/aggregation.md):** grouped sum, count, minimum, and maximum over completed predicates; host-registered pure functions and named aggregators.
- **[Vocabulary](reference/spl.md#the-predicate-vocabulary) and [verification](internals/verification.md):** predicate declarations, metadata, and non-semantic shape diagnostics. Lean models and Rust/Lean differential tests cover supported fragments.
- **[Trust-aware reasoning](guides/trust.md):** source attribution and weighted conclusions.
- **[Input](reference/spl.md) and [integration](integration/wasm.md):** Lisp-based SPL with variable, temporal, and trust directives; WebAssembly support for browsers and Node.js.

The linked pages describe current behavior and supported boundaries.

## Entry Points

[Getting Started](getting-started.md) covers CLI installation and a first theory.
The [Rust Library API](integration/rust.md#installation) describes library dependencies.
Its [basic example](integration/rust.md#basic-usage) constructs the penguin theory, sets superiority, and obtains conclusions with `Theory::reason()`.

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
