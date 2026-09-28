# Spindle-Rust

[![License: LGPL v3](https://img.shields.io/badge/License-LGPL_v3-blue.svg)](LICENSE)

A defeasible logic reasoning engine for rules, exceptions, and conflicting
conclusions. Write a theory in SPL (Spindle Lisp), ask what follows, and inspect
why a conclusion holds—or what prevents it. Use the command-line tool, embed the
Rust library, or run the same engine in a browser or Node.js through WebAssembly.

[Documentation](https://spindle-rust.anuna.io) ·
[SPL reference](docs/src/reference/spl.md) ·
[CLI reference](docs/src/reference/cli.md) ·
[Changelog](CHANGELOG.md)

## Quick start

Requires Rust 1.87 or later. From a checkout of this repository:

```sh
cargo install --path crates/spindle-cli
spindle reason examples/penguin.spl
```

The example says that birds normally fly, penguins normally do not, and the
penguin rule takes priority:

```lisp
(given bird)
(given penguin)

(normally r1 bird flies)
(normally r2 penguin (not flies))
(prefer r2 r1)
```

Output:

```text
Proved:

  (bird)
  (not (flies))
  (penguin)
```

A fact cannot be defeated by a default. A `normally` rule can: here the
preference resolves the conflict in favor of `(not flies)`. SPL output uses
canonical parentheses, so `(not (flies))` means the same thing.

Explore the same theory:

```sh
# Query a literal; quote SPL expressions for the shell
spindle query '(not flies)' examples/penguin.spl

# Explain a proof or inspect why a goal fails
spindle explain '(not flies)' examples/penguin.spl
spindle why-not flies examples/penguin.spl

# Inspect every proof tag, or get typed JSON for an application
spindle reason --detailed examples/penguin.spl
spindle reason --v2 examples/penguin.spl

# Validate the theory and inspect its predicate vocabulary
spindle validate examples/penguin.spl
spindle vocabulary examples/penguin.spl
```

The engine implements traditional ambiguity-blocking **DL(∂)** with four proof
tags: `+D` / `-D` for definite proof / refutation, and `+d` / `-d` for defeasible
proof / refutation. Negative tags need constructive proofs; unsupported cycles
can remain undecided. Default text output lists proved literals once; JSON
includes all tags unless filtered with `--positive`.

## Writing rules

| SPL form | Meaning |
|---|---|
| `(given bird)` | An unconditional fact |
| `(always r1 penguin bird)` | A strict implication |
| `(normally r2 bird flies)` | A default that can be defeated |
| `(except r3 injured (not flies))` | Block `flies` without proving `(not flies)` |
| `(prefer r3 r2)` | Give one rule priority over another |

Predicates take arguments, variables start with `?`, and rule bodies can combine
conditions with `and`. Arithmetic can bind values and filter matches. Save this
as `prices.spl`:

```lisp
(given (price widget 25))
(given (price pencil 3))

(normally discounted
  (and (price ?item ?price)
       (bind ?sale (- ?price 5))
       (> ?sale 0))
  (sale-price ?item ?sale))
```

```sh
spindle reason prices.spl
```

This derives `(sale-price widget 20)`. The pencil fails the positive-price guard.
Arithmetic supports integers, fixed-precision decimals, and finite floats.
See [variables](docs/src/guides/grounding.md) and
[arithmetic](docs/src/guides/arithmetic.md) for binding and numeric rules.

## Obligations and permissions

Use `must`, `may`, and `forbidden` for obligations, explicit permissions, and
prohibitions. For example, save this as `permission.spl`:

```lisp
(given hat)
(normally no-play () (forbidden play))
(normally hat-play hat (may play))
(prefer hat-play no-play)
```

```sh
spindle reason permission.spl
spindle query '(may play)' permission.spl
```

The original theory proves `(hat)` and `(may (play))`: the preferred permission
defeats the prohibition. Permission does not imply obligation or establish that
an action happened. `(forbidden play)` means `(must (not play))`.

Negation has scope: `(not (must play))` negates an obligation, while
`(must (not play))` obliges nonperformance. An obligation-headed defeater can
block an opposing obligation but cannot block an explicit permission.
See the [modal guide](docs/src/guides/modal.md) for conflict rules and examples.

## Asking what would be needed

`what-if` adds hypothetical facts without changing the original theory.
`abduce` proposes candidate fact sets; `requires` checks candidates by running
full reasoning. For a small example, save this as `access.spl`:

```lisp
(normally access (and member paid) admitted)
```

```sh
spindle what-if admitted access.spl --given member --given paid
spindle abduce admitted access.spl
spindle requires admitted access.spl --json
```

The hypothetical query proves `admitted`. Requirements can include adding
`member` and `paid`, or asserting the goal itself. Check `search_status` and
truncation diagnostics when using bounded search. Both search operators
recognize modal premises satisfied by constructive refutation.
See [query operators](docs/src/guides/queries.md).

## More capabilities

| Capability | Guide |
|---|---|
| Sum, count, minimum, and maximum over completed reasoning snapshots | [Aggregation](docs/src/guides/aggregation.md) |
| Source attribution, trust degrees, and diminishment from defeated challenges | [Trust](docs/src/guides/trust.md) |
| Temporal windows, reference-time filtering, and Allen interval relations | [Temporal reasoning](docs/src/guides/temporal.md) |
| Proof trees and natural-language, JSON, JSON-LD, and DOT explanations | [Explanations](docs/src/guides/explanations.md) |
| Predicate declarations, metadata, vocabulary, and shape diagnostics | [Inspecting a theory](docs/src/guides/inspect-theory.md) |
| Portable lookup functions and aggregators; pure Rust host functions | [Extensions](docs/aggregation-extensions.md) |

For example, this aggregate derives `(total 30)`:

```lisp
(given (amount 10))
(given (amount 20))
(normally total
  (agg ?sum sum ?value (amount ?value))
  (total ?sum))
```

Run `spindle reason examples/aggregation.spl` for a grouped example. Aggregation
currently supports integer/symbol predicates; modal, temporal, trust-weighted,
wildcard, decimal, and floating-point aggregate programs are rejected.

## Embed in Rust

Add `spindle-core` and `spindle-parser` using the
[dependency instructions](docs/src/integration/rust.md#installation), then parse
SPL and reason over the resulting theory:

```rust
use spindle_core::reason::reason;
use spindle_parser::parse_spl;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theory = parse_spl(r#"
        (given bird)
        (given penguin)
        (normally r1 bird flies)
        (normally r2 penguin (not flies))
        (prefer r2 r1)
    "#)?;

    for conclusion in reason(&theory)? {
        if conclusion.conclusion_type.is_positive() {
            println!("{} {}",
                conclusion.conclusion_type.symbol(),
                conclusion.literal.to_spl());
        }
    }
    Ok(())
}
```

Each conclusion has a proof tag, so a fact can appear at both `+D` and `+d`.
The [Rust reference](docs/src/integration/rust.md) covers programmatic theory
construction, preparation options, queries, and extension functions.

## Run in a browser

Install `wasm-pack`, then build from the repository root:

```sh
make wasm
```

Copy `crates/spindle-wasm/pkg` into your web app. Run this as a JavaScript module
beside `pkg`, served over HTTP:

```javascript
import init, { Spindle } from './pkg/spindle_wasm.js';

await init();
const spindle = new Spindle();
try {
    spindle.parseSpl(`
        (given bird)
        (given penguin)
        (normally r1 bird flies)
        (normally r2 penguin (not flies))
        (prefer r2 r1)
    `);

    const result = spindle.reasonV2();
    console.log(result.schema_version); // "spindle.reason.v2"
    for (const conclusion of result.conclusions) {
        if (conclusion.positive) {
            console.log(conclusion.conclusion_type, conclusion.literal_spl);
        }
    }

    console.log(spindle.query('(not flies)').status); // "provable"
    console.log(spindle.whyNot('flies').blockers);
} finally {
    spindle.free();
}
```

`reason()` and `reasonV2()` return structured envelopes with `conclusions`,
diagnostics, and statistics. V2 preserves typed term arguments in
`literal_struct`; `literal_spl` provides canonical SPL for display.
`requires(goal, maxSolutions)` returns verified requirements in the same
`spindle.requires.v2` envelope as the CLI.

Use `make wasm-node` for Node.js or `make wasm-bundler` for bundlers. See the
[WASM reference](docs/src/integration/wasm.md) for all methods and the
[browser guide](docs/src/guides/run-in-browser.md) for integration details.

## Development and verification

The Cargo workspace contains five crates:

| Crate | Purpose |
|---|---|
| `spindle-core` | Theory types, preparation, reasoning, queries, and explanations |
| `spindle-parser` | SPL lexer and parser |
| `spindle-cli` | The `spindle` command-line tool |
| `spindle-contract` | Shared CLI/WASM JSON contracts |
| `spindle-wasm` | JavaScript bindings through WebAssembly |

```sh
make build       # Build all crates
make test        # Workspace tests and doctests
make check       # Formatting and clippy; zero warnings
make bench       # Criterion benchmarks
```

Tests include regression cases, property tests, query and explanation checks,
and CLI/WASM comparisons. Lean models and differential suites cover supported
reasoning fragments, including aggregation and deontic reasoning. These provide
kernel-checked model proofs and executable comparisons with Rust; they do not
constitute a proof of the Rust implementation. See
[verification](lean/README.md) for prerequisites and commands.

To preview the documentation, use the mdBook version pinned in
[the documentation workflow](.forgejo/workflows/docs.yml):

```sh
mdbook serve docs
```

## Releases and license

The project is pre-1.0. Breaking changes bump the minor version (`0.y.0`);
backward-compatible changes and fixes bump the patch version (`0.y.z`). See the
[changelog](CHANGELOG.md) for release details and migration notes.

Spindle-Rust is part of the [SPINdle](https://research.csiro.au/bpli/tools/spindle/)
family, based on [spindle-racket](https://codeberg.org/anuna/spindle-racket)
v1.7.0. The original Java engine was developed at NICTA, later Data61/CSIRO.

Licensed under [LGPL-3.0-or-later](LICENSE).
